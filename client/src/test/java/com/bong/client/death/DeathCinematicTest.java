package com.bong.client.death;

import com.bong.client.hud.HudRenderCommand;
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
        assertEquals(List.of("生", "生", "生"), DeathRollUI.bambooSlipLabels(DeathCinematicState.RollResult.SURVIVE));
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
        assertEquals(0, NearDeathCollapsePlanner.qiEscapeDensityByHp(0.20));
        assertEquals(3, NearDeathCollapsePlanner.qiEscapeDensityByHp(0.04));
        assertTrue(NearDeathCollapsePlanner.meridianGlowOnSevered(true, 0.50));
        assertTrue(NearDeathCollapsePlanner.meridianGlowOnSevered(false, 0.09));
        assertEquals(8, NearDeathCollapsePlanner.surfaceCrackLines(0.04));
        assertTrue(NearDeathCollapsePlanner.collapseFreezeBeforeDeath(18L));
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
