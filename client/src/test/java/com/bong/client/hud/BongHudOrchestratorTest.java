package com.bong.client.hud;

import com.bong.client.botany.BotanySkillViewModel;
import com.bong.client.botany.HarvestSessionStore;
import com.bong.client.botany.HarvestSessionViewModel;
import com.bong.client.identity.IdentityPanelEntry;
import com.bong.client.identity.IdentityPanelState;
import com.bong.client.identity.IdentityPanelStateStore;
import com.bong.client.loop.HomeSequence;
import com.bong.client.npc.NpcInteractionLogStore;
import com.bong.client.npc.NpcMoodStore;
import com.bong.client.social.NicheGuardianStore;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillSetSnapshot;
import com.bong.client.skill.SkillSetStore;
import com.bong.client.state.NarrationState;
import com.bong.client.state.PlayerStateStore;
import com.bong.client.state.PlayerStateViewModel;
import com.bong.client.state.VisualEffectState;
import com.bong.client.state.ZoneState;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BongHudOrchestratorTest {
    private static final HudTextHelper.WidthMeasurer FIXED_WIDTH = text -> text == null ? 0 : text.length() * 6;

    @AfterEach
    void resetToastState() {
        BongToast.resetForTests();
        InventoryStateStore.resetForTests();
        HarvestSessionStore.resetForTests();
        SkillSetStore.resetForTests();
        PlayerStateStore.resetForTests();
        IdentityPanelStateStore.resetForTest();
        TargetInfoStateStore.resetForTests();
        NpcMoodStore.clearAll();
        NpcInteractionLogStore.resetForTests();
        NicheGuardianStore.resetForTests();
        com.bong.client.tsy.TsyBossHealthStore.resetForTests();
        com.bong.client.tsy.TsyDeathVfxStore.resetForTests();
        SearchHudStateStore.resetForTests();
        HudImmersionMode.resetForTests();
        HudLayoutPreferenceStore.resetForTests();
        ForgeProgressHudPlanner.resetForTests();
        HomeSequence.resetForTests();
    }

    @Test
    void emptyStateHasNoPersistentPlaceholder() {
        List<HudRenderCommand> commands = BongHudOrchestrator.buildCommands(
            BongHudStateSnapshot.empty(),
            0L,
            FIXED_WIDTH,
            220
        );

        assertTrue(commands.isEmpty(), "没有有效状态时不再显示连接占位文字");
    }


    @Test
    void localNegativePressureDoesNotProducePersistentText() {
        PlayerStateStore.replace(PlayerStateViewModel.create(
            "Solidify",
            "offline:Azure",
            80.0,
            100.0,
            0.0,
            0.5,
            PlayerStateViewModel.PowerBreakdown.empty(),
            PlayerStateViewModel.SocialSnapshot.empty(),
            "rift_mouth_north_001",
            "渊口荒丘",
            0.05,
            -0.8
        ));
        BongHudStateSnapshot snapshot = BongHudStateSnapshot.create(
            ZoneState.create("rift_mouth_north_001", "渊口荒丘", 0.05, 5, 100L),
            NarrationState.empty(),
            VisualEffectState.none()
        );

        List<HudRenderCommand> commands = BongHudOrchestrator.buildCommands(snapshot, 250L, FIXED_WIDTH, 220);

        assertTrue(commands.stream().noneMatch(cmd -> cmd.text().startsWith("灵压")), "常驻灵压数字已移除");
    }

    @Test
    void localNegativePressureAddsVisualVignette() {
        PlayerStateStore.replace(PlayerStateViewModel.create(
            "Solidify",
            "offline:Azure",
            80.0,
            100.0,
            0.0,
            0.5,
            PlayerStateViewModel.PowerBreakdown.empty(),
            PlayerStateViewModel.SocialSnapshot.empty(),
            "tsy_lingxu_01_deep",
            "灵墟深层",
            -0.9,
            -0.9
        ));

        List<HudRenderCommand> commands = BongHudOrchestrator.buildCommands(
            BongHudStateSnapshot.empty(),
            250L,
            FIXED_WIDTH,
            220,
            320,
            180
        );

        assertTrue(commands.stream().anyMatch(HudRenderCommand::isEdgeVignette));
    }

    @Test
    void oversizedZoneAndToastTextAreClippedSafely() {
        NarrationState warningToast = NarrationState.create(
            "zone",
            "jade_valley",
            "A decree stretches far beyond the narrow HUD bounds and must be clipped safely.",
            "era_decree"
        );
        BongToast.show(warningToast, 0L);

        BongHudStateSnapshot snapshot = BongHudStateSnapshot.create(
            ZoneState.create("jade_valley", "Ancient Jade Valley of Unending Mist and Starfall Echoes", 0.74, 3, 100L),
            NarrationState.create("zone", "jade_valley", "A quiet breeze passes through the valley.", "narration"),
            VisualEffectState.none()
        );

        List<HudRenderCommand> commands = BongHudOrchestrator.buildCommands(snapshot, 0L, FIXED_WIDTH, 72);
        assertEquals(1, commands.size());
        HudRenderCommand toast = commands.get(0);
        assertTrue(toast.isToast());
        assertTrue(FIXED_WIDTH.measure(toast.text()) <= 72);

    }

    @Test
    void activeToastSurvivesLaterNonToastNarrationUntilExpiry() {
        NarrationState warningToast = NarrationState.create("broadcast", null, "雷劫将至，速速退避。", "system_warning");
        BongToast.show(warningToast, 100L);

        BongHudStateSnapshot laterSnapshot = BongHudStateSnapshot.create(
            ZoneState.empty(),
            NarrationState.create("broadcast", null, "风声微动", "perception"),
            VisualEffectState.none()
        );

        List<HudRenderCommand> commands = BongHudOrchestrator.buildCommands(laterSnapshot, 4_000L, FIXED_WIDTH, 220);

        assertEquals(1, commands.size());
        assertEquals(HudRenderLayer.TOAST, commands.get(0).layer());
        assertTrue(commands.get(0).text().startsWith("天道警示"));
    }

    @Test
    void overlyNarrowWidthDropsOversizedContent() {
        BongHudStateSnapshot snapshot = BongHudStateSnapshot.create(
            ZoneState.create("jade_valley", "Ancient Jade Valley", 0.8, 2, 100L),
            NarrationState.create("zone", "jade_valley", "Danger rises swiftly.", "system_warning"),
            VisualEffectState.none()
        );

        List<HudRenderCommand> commands = BongHudOrchestrator.buildCommands(snapshot, 0L, FIXED_WIDTH, 2);

        assertTrue(commands.isEmpty());
    }

    @Test
    void overweightWarningRemainsConditional() {
        InventoryStateStore.applyAuthoritativeSnapshot(
            InventoryModel.builder()
                .containers(InventoryModel.DEFAULT_CONTAINERS)
                .weight(60.0, 50.0)
                .build(),
            3L
        );

        List<HudRenderCommand> commands = BongHudOrchestrator.buildCommands(
            BongHudStateSnapshot.empty(),
            0L,
            FIXED_WIDTH,
            220
        );

        assertEquals(1, commands.size());
        assertEquals(HudRenderLayer.OVERWEIGHT, commands.get(0).layer());
        assertTrue(commands.get(0).text().contains("超载"));
    }

    @Test
    void identityDataDoesNotCreatePersistentLabel() {
        IdentityPanelStateStore.replace(new IdentityPanelState(
            0,
            100L,
            0L,
            List.of(new IdentityPanelEntry(0, "白面", 0, false, List.of()))
        ));

        List<HudRenderCommand> commands = BongHudOrchestrator.buildCommands(
            BongHudStateSnapshot.empty(),
            0L,
            FIXED_WIDTH,
            220,
            320,
            180
        );

        assertTrue(commands.stream().noneMatch(cmd -> "[#0] 白面".equals(cmd.text())), "身份仍可在面板查看，但不再常驻 HUD");
    }

    @Test
    void activeBotanySessionAddsBotanyLayerCommands() {
        HarvestSessionStore.replace(HarvestSessionViewModel.create(
            "session-botany-01",
            "plant-1",
            "开脉草",
            "ning_mai_cao",
            null,
            0.0,
            true,
            false,
            false,
            false,
            "晨露未散",
            10L
        ));
        SkillSetStore.updateEntry(
            SkillId.HERBALISM,
            new SkillSetSnapshot.Entry(2, 90L, 120L, 90L, 10, 0L, 0L)
        );

        List<HudRenderCommand> commands = BongHudOrchestrator.buildCommands(
            BongHudStateSnapshot.empty(),
            0L,
            FIXED_WIDTH,
            220,
            320,
            180
        );

        assertTrue(commands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.BOTANY));
    }

    @Test
    void hudModeSwitchesHideCombatOnlyLayersInPeaceAndQuickBarInCultivation() {
        CombatHudSnapshot combat = CombatHudSnapshot.create(
            com.bong.client.combat.CombatHudState.create(
                0.8f,
                0.7f,
                0.4f,
                com.bong.client.combat.DerivedAttrFlags.none()
            ),
            null,
            com.bong.client.combat.QuickSlotConfig.empty(),
            com.bong.client.combat.SkillBarConfig.empty(),
            -1,
            com.bong.client.combat.CastState.idle(),
            com.bong.client.combat.UnifiedEventStream.empty(),
            com.bong.client.combat.SpellVolumeState.idle(),
            com.bong.client.combat.store.CarrierStateStore.State.NONE,
            com.bong.client.combat.DefenseWindowState.idle(),
            com.bong.client.combat.UnlockedStyles.none()
        );

        List<HudRenderCommand> combatCommands = BongHudOrchestrator.buildCommands(
            BongHudStateSnapshot.empty(), combat, 1_000L, FIXED_WIDTH, 220, 320, 180
        );
        assertTrue(combatCommands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.QUICK_BAR));
        assertTrue(combatCommands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.MINI_BODY));

        HudImmersionMode.resetForTests();
        List<HudRenderCommand> peaceCommands = BongHudOrchestrator.buildCommands(
            BongHudStateSnapshot.empty(), CombatHudSnapshot.empty(), 12_000L, FIXED_WIDTH, 220, 320, 180
        );
        BongHudStateSnapshot meditation = BongHudStateSnapshot.create(
            ZoneState.empty(),
            NarrationState.empty(),
            VisualEffectState.create("meditation_calm", 1.0, 20_000L, 12_000L)
        );
        List<HudRenderCommand> cultivationCommands = BongHudOrchestrator.buildCommands(
            meditation, CombatHudSnapshot.empty(), 12_200L, FIXED_WIDTH, 220, 320, 180
        );
        assertTrue(cultivationCommands.stream().noneMatch(cmd -> cmd.layer() == HudRenderLayer.QUICK_BAR));
    }

    @Test
    void completedSearchFlashUsesExactTtlInFinalHudCommandFlow() {
        long startedAtNanos = 10_000L;
        SearchHudStateStore.markCompletedAtNanos("残棺", startedAtNanos);

        List<HudRenderCommand> beforeBoundary = buildCombatFrame(
            HudRuntimeContext.empty(),
            startedAtNanos + SearchHudStateStore.COMPLETED_FLASH_TTL_NANOS - 1L
        );
        assertTrue(
            beforeBoundary.stream().anyMatch(cmd ->
                cmd.layer() == HudRenderLayer.SEARCH_PROGRESS && "搜刮完成：残棺".equals(cmd.text())
            ),
            "completed flash 在 TTL-1ns 必须仍进入最终 SEARCH_PROGRESS 命令流，实际命令=" + beforeBoundary
        );

        List<HudRenderCommand> atBoundary = buildCombatFrame(
            HudRuntimeContext.empty(),
            startedAtNanos + SearchHudStateStore.COMPLETED_FLASH_TTL_NANOS
        );
        assertTrue(
            atBoundary.stream().noneMatch(cmd -> cmd.layer() == HudRenderLayer.SEARCH_PROGRESS),
            "completed flash 在 TTL 精确边界必须从最终命令流消失，实际命令=" + atBoundary
        );

        List<HudRenderCommand> afterBoundary = buildCombatFrame(
            HudRuntimeContext.empty(),
            startedAtNanos + SearchHudStateStore.COMPLETED_FLASH_TTL_NANOS + 1L
        );
        assertTrue(
            afterBoundary.stream().noneMatch(cmd -> cmd.layer() == HudRenderLayer.SEARCH_PROGRESS),
            "completed flash 在 TTL+1ns 必须保持消失，不能重新进入最终命令流，实际命令=" + afterBoundary
        );
    }

    @Test
    void abortedSearchFlashUsesExactTtlAcrossCombatAndMovementContexts() {
        long startedAtNanos = 20_000L;
        SearchHudStateStore.markAbortedAtNanos("残柜", "combat", startedAtNanos);

        List<HudRenderCommand> beforeBoundary = buildCombatFrame(
            new HudRuntimeContext(45.0, 12.0, 64.0, -8.0, false, List.of()),
            startedAtNanos + SearchHudStateStore.ABORTED_FLASH_TTL_NANOS - 1L
        );
        assertTrue(
            beforeBoundary.stream().anyMatch(cmd ->
                cmd.layer() == HudRenderLayer.SEARCH_PROGRESS && "搜刮中断：进入战斗".equals(cmd.text())
            ),
            "aborted flash 在 TTL-1ns 必须仍进入最终 SEARCH_PROGRESS 命令流，实际命令=" + beforeBoundary
        );

        List<HudRenderCommand> atBoundary = buildCombatFrame(
            new HudRuntimeContext(225.0, 18.5, 64.0, -2.5, false, List.of()),
            startedAtNanos + SearchHudStateStore.ABORTED_FLASH_TTL_NANOS
        );
        assertTrue(
            atBoundary.stream().noneMatch(cmd -> cmd.layer() == HudRenderLayer.SEARCH_PROGRESS),
            "aborted flash 在移动与战斗上下文变化后的 TTL 精确边界必须消失，实际命令=" + atBoundary
        );

        List<HudRenderCommand> afterBoundary = buildCombatFrame(
            new HudRuntimeContext(270.0, 21.0, 64.0, 1.0, false, List.of()),
            startedAtNanos + SearchHudStateStore.ABORTED_FLASH_TTL_NANOS + 1L
        );
        assertTrue(
            afterBoundary.stream().noneMatch(cmd -> cmd.layer() == HudRenderLayer.SEARCH_PROGRESS),
            "aborted flash 在 TTL+1ns 必须保持消失，不能因上下文变化重新出现，实际命令=" + afterBoundary
        );
    }

    private static List<HudRenderCommand> buildCombatFrame(
        HudRuntimeContext runtimeContext,
        long nowNanos
    ) {
        CombatHudSnapshot combat = CombatHudSnapshot.create(
            com.bong.client.combat.CombatHudState.create(
                0.8f,
                0.7f,
                0.4f,
                com.bong.client.combat.DerivedAttrFlags.none()
            ),
            null,
            com.bong.client.combat.QuickSlotConfig.empty(),
            com.bong.client.combat.SkillBarConfig.empty(),
            -1,
            com.bong.client.combat.CastState.idle(),
            com.bong.client.combat.UnifiedEventStream.empty(),
            com.bong.client.combat.SpellVolumeState.idle(),
            com.bong.client.combat.store.CarrierStateStore.State.NONE,
            com.bong.client.combat.DefenseWindowState.idle(),
            com.bong.client.combat.UnlockedStyles.none()
        );
        return BongHudOrchestrator.buildCommands(
            BongHudStateSnapshot.empty(),
            combat,
            1_000L,
            FIXED_WIDTH,
            220,
            320,
            180,
            null,
            runtimeContext,
            nowNanos
        );
    }
}
