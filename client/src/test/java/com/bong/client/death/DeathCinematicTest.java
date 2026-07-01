package com.bong.client.death;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudTextHelper;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class DeathCinematicTest {
    @Test
    void payloadParserReadsCinematicContract() {
        var obj = JsonParser.parseString("""
            {
              "v": 1,
              "character_id": "offline:Azure",
              "phase": "roll",
              "phase_tick": 30,
              "phase_duration_ticks": 80,
              "total_elapsed_ticks": 110,
              "total_duration_ticks": 380,
              "roll": {
                "probability": 0.65,
                "threshold": 0.65,
                "luck_value": 0.42,
                "result": "pending"
              },
              "insight_text": ["劫未尽", "坍缩渊，概不赊欠。"],
              "is_final": false,
              "death_number": 4,
              "zone_kind": "negative",
              "tsy_death": true,
              "rebirth_weakened_ticks": 3600,
              "skip_predeath": false
            }
            """).getAsJsonObject();

        DeathCinematicState state = DeathCinematicPayloadParser.parse(obj, 1_000L);

        assertTrue(state.active());
        assertEquals("offline:Azure", state.characterId());
        assertEquals(DeathCinematicState.Phase.ROLL, state.phase());
        assertEquals(DeathCinematicState.RollResult.PENDING, state.roll().result());
        assertEquals(0.65, state.roll().probability(), 1e-9);
        assertEquals(List.of("劫未尽", "坍缩渊，概不赊欠。"), state.insightText());
        assertTrue(state.tsyDeath());
        assertEquals(1_000L, state.receivedAtMillis());
    }

    @Test
    void payloadParserRejectsMissingVersionAndRequiredCharacter() {
        assertFalse(DeathCinematicPayloadParser.parse(null, 1_000L).active());
        assertFalse(DeathCinematicPayloadParser.parse(JsonParser.parseString("{}").getAsJsonObject(), 1_000L).active());
        assertFalse(DeathCinematicPayloadParser.parse(
            JsonParser.parseString("{\"v\":2,\"character_id\":\"offline:Azure\"}").getAsJsonObject(),
            1_000L
        ).active());
        assertFalse(DeathCinematicPayloadParser.parse(
            JsonParser.parseString("{\"v\":1,\"phase\":\"roll\"}").getAsJsonObject(),
            1_000L
        ).active());
    }

    @Test
    void payloadParserClampsDurationsDeathNumberAndUnknownEnums() {
        var obj = JsonParser.parseString("""
            {
              "v": 1,
              "character_id": "offline:Azure",
              "phase": "unknown_phase",
              "phase_tick": "bad",
              "phase_duration_ticks": 0,
              "total_elapsed_ticks": 5,
              "total_duration_ticks": 0,
              "roll": {
                "probability": 2.0,
                "threshold": -1.0,
                "luck_value": 0.42,
                "result": "unknown_result"
              },
              "insight_text": [1, "", "劫未尽"],
              "is_final": false,
              "death_number": 9999999999,
              "zone_kind": "ordinary",
              "tsy_death": false,
              "rebirth_weakened_ticks": -3,
              "skip_predeath": false
            }
            """).getAsJsonObject();

        DeathCinematicState state = DeathCinematicPayloadParser.parse(obj, 1_000L);

        assertTrue(state.active());
        assertEquals(DeathCinematicState.Phase.PREDEATH, state.phase());
        assertEquals(DeathCinematicState.RollResult.PENDING, state.roll().result());
        assertEquals(1L, state.phaseDurationTicks());
        assertEquals(1L, state.totalDurationTicks());
        assertEquals(Integer.MAX_VALUE, state.deathNumber());
        assertEquals(List.of("劫未尽"), state.insightText());
        assertEquals(0L, state.rebirthWeakenedTicks());
        assertEquals(1_000L, state.receivedAtMillis());
    }

    @Test
    void localClockAdvancesPhaseSequenceAfterPayloadReceipt() {
        DeathCinematicState state = baseState(
            DeathCinematicState.Phase.PREDEATH,
            0L,
            60L,
            0L,
            380L,
            false,
            1,
            false,
            1_000L
        );

        DeathCinematicState advanced = state.advancedTo(10_000L);

        assertEquals(DeathCinematicState.Phase.INSIGHT_OVERLAY, advanced.phase());
        assertEquals(20L, advanced.phaseTick());
        assertEquals(120L, advanced.phaseDurationTicks());
    }

    @Test
    void fifthNonFinalDeathSkipsToRoll() {
        DeathCinematicState state = baseState(
            DeathCinematicState.Phase.ROLL,
            0L,
            40L,
            0L,
            200L,
            false,
            5,
            true,
            1_000L
        );

        DeathCinematicState advanced = state.advancedTo(1_500L);

        assertEquals(DeathCinematicState.Phase.ROLL, advanced.phase());
        assertEquals(10L, advanced.phaseTick());
        assertEquals(40L, advanced.phaseDurationTicks());
    }

    @Test
    void rollProbabilityScrollsToActualAndLabelsResult() {
        DeathCinematicState state = baseState(
            DeathCinematicState.Phase.ROLL,
            38L,
            100L,
            118L,
            380L,
            false,
            1,
            false,
            1_000L
        );

        assertEquals(0.65, DeathRollUI.displayedProbability(state), 1e-9);
        assertEquals(List.of("?", "?", "?"), DeathRollUI.bambooSlipLabels(null));
        assertEquals(List.of("?", "?", "?"), DeathRollUI.bambooSlipLabels(DeathCinematicState.RollResult.PENDING));
        assertEquals(List.of("生", "生", "生"), DeathRollUI.bambooSlipLabels(DeathCinematicState.RollResult.SURVIVE));
        assertEquals(List.of("落", "落", "生"), DeathRollUI.bambooSlipLabels(DeathCinematicState.RollResult.FALL));
        assertEquals(List.of("终", "终", "碎"), DeathRollUI.bambooSlipLabels(DeathCinematicState.RollResult.FINAL));
    }

    @Test
    void rendererDispatchesByAdvancedPhase() {
        DeathCinematicState state = baseState(
            DeathCinematicState.Phase.PREDEATH,
            0L,
            60L,
            0L,
            380L,
            false,
            1,
            false,
            1_000L
        );

        List<HudRenderCommand> commands = DeathCinematicRenderer.buildCommands(state, 4_000L, 320, 180);

        assertTrue(commands.stream().anyMatch(HudRenderCommand::isRect));
    }

    @Test
    void screenShatterCreatesSixteenFragments() {
        assertEquals(16, ScreenShatterEffect.fragments(320, 180, 4L).size());
    }

    @Test
    void nearDeathCollapseThresholdsMatchThreeLayers() {
        assertEquals(3, NearDeathCollapsePlanner.qiEscapeDensityByHp(0.0));
        assertEquals(0, NearDeathCollapsePlanner.qiEscapeDensityByHp(1.0));
        assertEquals(0, NearDeathCollapsePlanner.qiEscapeDensityByHp(0.20));
        assertEquals(3, NearDeathCollapsePlanner.qiEscapeDensityByHp(0.1999));
        assertEquals(0, NearDeathCollapsePlanner.qiEscapeDensityByHp(0.2001));
        assertEquals(3, NearDeathCollapsePlanner.qiEscapeDensityByHp(0.04));
        assertTrue(NearDeathCollapsePlanner.meridianGlowOnSevered(true, 0.50));
        assertFalse(NearDeathCollapsePlanner.meridianGlowOnSevered(false, 0.10));
        assertFalse(NearDeathCollapsePlanner.meridianGlowOnSevered(false, 0.11));
        assertTrue(NearDeathCollapsePlanner.meridianGlowOnSevered(false, 0.09));
        assertEquals(0, NearDeathCollapsePlanner.surfaceCrackLines(0.05));
        assertEquals(0, NearDeathCollapsePlanner.surfaceCrackLines(0.06));
        assertEquals(8, NearDeathCollapsePlanner.surfaceCrackLines(0.0499));
        assertEquals(8, NearDeathCollapsePlanner.surfaceCrackLines(0.04));
        assertFalse(NearDeathCollapsePlanner.collapseFreezeBeforeDeath(13L));
        assertTrue(NearDeathCollapsePlanner.collapseFreezeBeforeDeath(14L));
        assertTrue(NearDeathCollapsePlanner.collapseFreezeBeforeDeath(18L));
        assertTrue(NearDeathCollapsePlanner.collapseFreezeBeforeDeath(20L));
        assertFalse(NearDeathCollapsePlanner.collapseFreezeBeforeDeath(21L));
    }

    // ──────────────────────────────────────────────────────────────────────
    // F15 fix — NearDeathCollapsePlanner.buildCommands() 之前只 emit
    // screenTint+edgeVignette+text 三条固定命令，qiEscapeDensityByHp /
    // meridianGlowOnSevered / surfaceCrackLines / collapseFreezeBeforeDeath
    // 四个已测纯函数从未被接进渲染输出。以下锁住接线后的可观察命令契约。
    // ──────────────────────────────────────────────────────────────────────

    @Test
    void nearDeathCollapseHighHpAndNoFreezeEmitsOnlyBaseThreeCommands() {
        // phaseTick=0/phaseDurationTicks=100 → progress=0 → hpPercent(代理)=1.0：
        // 高于 qiEscapeDensityByHp/surfaceCrackLines 的 0.20/0.05 阈值，且 phaseTick=0 不在
        // collapseFreezeBeforeDeath 的 [14,20] 冻结窗口内 —— 不应新增任何裂痕/外泄 rect。
        DeathCinematicState state = baseState(
            DeathCinematicState.Phase.PREDEATH, 0L, 100L, 0L, 380L, false, 1, false, 1_000L
        );

        List<HudRenderCommand> commands = NearDeathCollapsePlanner.buildCommands(state, 320, 180);

        assertEquals(
            3,
            commands.size(),
            "expected exactly 3 commands (tint+vignette+text) at hp≈1.0 with no severed/freeze signal, actual: "
                + commands.size() + " -> " + commands
        );
        assertTrue(commands.get(0).isScreenTint());
        assertTrue(commands.get(1).isEdgeVignette());
        assertTrue(commands.get(2).isText());
    }

    @Test
    void nearDeathCollapseLowHpAddsThreeQiEscapeRectsWithoutFreeze() {
        // phaseTick=900/phaseDurationTicks=1000 → progress=0.9 → hpPercent(代理)=0.10：
        // < 0.20 阈值 → qiEscapeDensityByHp 恒为 3；>= 0.05 → surfaceCrackLines 仍为 0；
        // phaseTick=900 不在冻结窗口 → escape rect alpha 用未冻结常量 ESCAPE_ALPHA。
        DeathCinematicState state = baseState(
            DeathCinematicState.Phase.PREDEATH, 900L, 1_000L, 0L, 380L, false, 1, false, 1_000L
        );

        List<HudRenderCommand> commands = NearDeathCollapsePlanner.buildCommands(state, 320, 180);

        int expectedEscapeColor = HudTextHelper.withAlpha(
            NearDeathCollapsePlanner.QI_COLOR, NearDeathCollapsePlanner.ESCAPE_ALPHA
        );
        long escapeRectCount = commands.stream()
            .filter(HudRenderCommand::isRect)
            .filter(c -> c.color() == expectedEscapeColor)
            .count();

        assertEquals(
            3,
            escapeRectCount,
            "expected 3 qi-escape rects at hpPercent=0.10 (qiEscapeDensityByHp(0.10)=3) using the non-frozen "
                + "escape alpha, actual matching rects: " + escapeRectCount + " in " + commands
        );
        assertEquals(
            6,
            commands.size(),
            "expected tint+vignette+3 escape rects+text = 6 total commands, actual: " + commands.size()
        );
    }

    @Test
    void nearDeathCollapseVeryLowHpAddsEscapeAndCrackRects() {
        // phaseTick=200/phaseDurationTicks=200 → progress=1.0 → hpPercent(代理)=0.0:
        // both qiEscapeDensityByHp(0.0)=3 and surfaceCrackLines(0.0)=8 fire; phaseTick=200
        // is outside the [14,20] freeze window → both use their non-frozen alpha constants.
        DeathCinematicState state = baseState(
            DeathCinematicState.Phase.PREDEATH, 200L, 200L, 0L, 380L, false, 1, false, 1_000L
        );

        List<HudRenderCommand> commands = NearDeathCollapsePlanner.buildCommands(state, 320, 180);

        int expectedEscapeColor = HudTextHelper.withAlpha(
            NearDeathCollapsePlanner.QI_COLOR, NearDeathCollapsePlanner.ESCAPE_ALPHA
        );
        int expectedCrackColor = HudTextHelper.withAlpha(
            NearDeathCollapsePlanner.SURFACE_COLOR, NearDeathCollapsePlanner.CRACK_ALPHA
        );
        long escapeRectCount = commands.stream().filter(HudRenderCommand::isRect)
            .filter(c -> c.color() == expectedEscapeColor).count();
        long crackRectCount = commands.stream().filter(HudRenderCommand::isRect)
            .filter(c -> c.color() == expectedCrackColor).count();

        assertEquals(3, escapeRectCount, "expected 3 qi-escape rects at hpPercent=0.0, actual: " + escapeRectCount);
        assertEquals(8, crackRectCount, "expected 8 surface-crack rects at hpPercent=0.0, actual: " + crackRectCount);
        // tint + vignette + 3 escape + 8 crack + text = 14
        assertEquals(14, commands.size(), "expected 14 total commands, actual: " + commands.size() + " -> " + commands);
    }

    @Test
    void nearDeathCollapseFreezeWindowLocksRectsToFrozenAlpha() {
        // phaseTick=15/phaseDurationTicks=15 → progress clamps to 1.0 → hpPercent=0.0
        // (same density as the non-frozen case above), but phaseTick=15 IS inside
        // collapseFreezeBeforeDeath's [14,20] window → both escape and crack rects must
        // use FROZEN_ALPHA instead of their normal progress-driven alpha.
        DeathCinematicState frozenState = baseState(
            DeathCinematicState.Phase.PREDEATH, 15L, 15L, 0L, 380L, false, 1, false, 1_000L
        );

        List<HudRenderCommand> commands = NearDeathCollapsePlanner.buildCommands(frozenState, 320, 180);

        int frozenEscapeColor = HudTextHelper.withAlpha(
            NearDeathCollapsePlanner.QI_COLOR, NearDeathCollapsePlanner.FROZEN_ALPHA
        );
        int frozenCrackColor = HudTextHelper.withAlpha(
            NearDeathCollapsePlanner.SURFACE_COLOR, NearDeathCollapsePlanner.FROZEN_ALPHA
        );
        long frozenEscapeCount = commands.stream().filter(HudRenderCommand::isRect)
            .filter(c -> c.color() == frozenEscapeColor).count();
        long frozenCrackCount = commands.stream().filter(HudRenderCommand::isRect)
            .filter(c -> c.color() == frozenCrackColor).count();

        assertEquals(
            3, frozenEscapeCount,
            "expected 3 qi-escape rects locked to FROZEN_ALPHA inside the [14,20] freeze window, actual: "
                + frozenEscapeCount + " in " + commands
        );
        assertEquals(
            8, frozenCrackCount,
            "expected 8 surface-crack rects locked to FROZEN_ALPHA inside the freeze window, actual: "
                + frozenCrackCount + " in " + commands
        );
    }

    @Test
    void nearDeathCollapseFinalDeathForcesMeridianGlowEvenAtFullHp() {
        // finalDeath=true is used as the "meridian already severed" surrogate signal
        // (DeathCinematicState has no literal hasSeveredMeridian field). At hp≈1.0 the
        // hp-driven branch of meridianGlowOnSevered would be false, so this isolates the
        // finalDeath-driven branch: the edge vignette must switch from QI_COLOR to
        // MERIDIAN_COLOR-derived alpha.
        DeathCinematicState finalDeathState = baseState(
            DeathCinematicState.Phase.PREDEATH, 0L, 100L, 0L, 380L, true, 1, false, 1_000L
        );
        DeathCinematicState nonFinalState = baseState(
            DeathCinematicState.Phase.PREDEATH, 0L, 100L, 0L, 380L, false, 1, false, 1_000L
        );

        List<HudRenderCommand> finalCommands = NearDeathCollapsePlanner.buildCommands(finalDeathState, 320, 180);
        List<HudRenderCommand> nonFinalCommands = NearDeathCollapsePlanner.buildCommands(nonFinalState, 320, 180);

        int expectedMeridianVignette = HudTextHelper.withAlpha(NearDeathCollapsePlanner.MERIDIAN_COLOR, 80);
        int expectedQiVignette = HudTextHelper.withAlpha(NearDeathCollapsePlanner.QI_COLOR, 80);

        assertEquals(
            expectedMeridianVignette,
            finalCommands.get(1).color(),
            "expected the edge vignette to use MERIDIAN_COLOR-derived alpha when finalDeath=true, actual: 0x"
                + Integer.toHexString(finalCommands.get(1).color())
        );
        assertEquals(
            expectedQiVignette,
            nonFinalCommands.get(1).color(),
            "expected the edge vignette to stay QI_COLOR-derived at hp≈1.0 when finalDeath=false, actual: 0x"
                + Integer.toHexString(nonFinalCommands.get(1).color())
        );
    }

    @Test
    void insightAndRebirthRenderExpectedNarration() {
        DeathCinematicState insight = baseState(
            DeathCinematicState.Phase.INSIGHT_OVERLAY,
            60L,
            120L,
            220L,
            380L,
            false,
            1,
            false,
            1_000L
        );
        assertEquals(2, InsightOverlayRenderer.visibleLineCount(insight));
        assertEquals(0, InsightOverlayRenderer.visibleLineCount(null));
        assertEquals(0, InsightOverlayRenderer.visibleLineCount(DeathCinematicState.INACTIVE));
        assertEquals(0, InsightOverlayRenderer.visibleLineCount(new DeathCinematicState(
            true,
            "offline:Azure",
            DeathCinematicState.Phase.INSIGHT_OVERLAY,
            0L,
            120L,
            0L,
            380L,
            new DeathCinematicState.Roll(0.65, 0.65, 0.42, DeathCinematicState.RollResult.SURVIVE),
            List.of(),
            false,
            1,
            "ordinary",
            false,
            3_600L,
            false,
            1_000L
        )));
        assertEquals(1, InsightOverlayRenderer.visibleLineCount(
            baseState(DeathCinematicState.Phase.INSIGHT_OVERLAY, 0L, 120L, 200L, 380L, false, 1, false, 1_000L)
        ));
        assertEquals(2, InsightOverlayRenderer.visibleLineCount(
            baseState(DeathCinematicState.Phase.INSIGHT_OVERLAY, 79L, 120L, 200L, 380L, false, 1, false, 1_000L)
        ));
        assertEquals(3, InsightOverlayRenderer.visibleLineCount(
            baseState(DeathCinematicState.Phase.INSIGHT_OVERLAY, 120L, 120L, 200L, 380L, false, 1, false, 1_000L)
        ));
        assertFalse(InsightOverlayRenderer.usesWarningColor("幸运数字是三"));
        assertTrue(InsightOverlayRenderer.usesWarningColor("此次运数：35%。下次 20%。"));
        assertTrue(InsightOverlayRenderer.usesWarningColor("坍缩渊，概不赊欠。"));
        assertTrue(RebirthCinematicRenderer.buildCommands(
                baseState(DeathCinematicState.Phase.REBIRTH, 20L, 60L, 340L, 380L, false, 1, false, 1_000L),
                320,
                180
            ).stream().anyMatch(command -> command.text().contains("虚弱")));
    }

    @Test
    void finalWordsUseDedicatedOverlayOnFinalDeath() {
        DeathCinematicState state = baseState(
            DeathCinematicState.Phase.INSIGHT_OVERLAY,
            20L,
            120L,
            200L,
            380L,
            true,
            1,
            false,
            1_000L
        );

        List<HudRenderCommand> commands = DeathCinematicRenderer.buildCommands(state, 1_000L, 320, 180);

        assertTrue(commands.stream().anyMatch(command -> command.text().contains("终焉之言")));
    }

    private static DeathCinematicState baseState(
        DeathCinematicState.Phase phase,
        long phaseTick,
        long phaseDurationTicks,
        long totalElapsedTicks,
        long totalDurationTicks,
        boolean finalDeath,
        int deathNumber,
        boolean skipPredeath,
        long receivedAtMillis
    ) {
        return new DeathCinematicState(
            true,
            "offline:Azure",
            phase,
            phaseTick,
            phaseDurationTicks,
            totalElapsedTicks,
            totalDurationTicks,
            new DeathCinematicState.Roll(0.65, 0.65, 0.42, DeathCinematicState.RollResult.SURVIVE),
            List.of("劫未尽", "坍缩渊，概不赊欠。", "你还活着。代价已付。"),
            finalDeath,
            deathNumber,
            "ordinary",
            false,
            3_600L,
            skipPredeath,
            receivedAtMillis
        );
    }
}
