package com.bong.client.hud;

import com.bong.client.botany.BotanySkillViewModel;
import com.bong.client.botany.HarvestSessionStore;
import com.bong.client.botany.HarvestSessionViewModel;
import com.bong.client.identity.IdentityPanelEntry;
import com.bong.client.identity.IdentityPanelState;
import com.bong.client.identity.IdentityPanelStateStore;
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
        HudImmersionMode.resetForTests();
        ForgeProgressHudPlanner.resetForTests();
    }

    @Test
    void emptyStateBuildsBaselineOnly() {
        List<HudRenderCommand> commands = BongHudOrchestrator.buildCommands(
            BongHudStateSnapshot.empty(),
            0L,
            FIXED_WIDTH,
            220
        );

        assertEquals(1, commands.size());
        assertEquals(HudRenderLayer.BASELINE, commands.get(0).layer());
        assertEquals(BongHudOrchestrator.BASELINE_LABEL, commands.get(0).text());
        assertEquals(10, commands.get(0).x());
        assertEquals(10, commands.get(0).y());
    }

    @Test
    void renderOrderStaysBaselineZoneToastVisual() {
        BongHudStateSnapshot snapshot = BongHudStateSnapshot.create(
            ZoneState.create("jade_valley", "Jade Valley", 0.74, 3, 100L),
            NarrationState.create("zone", "jade_valley", "The valley formation is shifting.", "system_warning"),
            VisualEffectState.create("fog_tint", 0.75, 1_000L, 0L)
        );
        BongToast.show(snapshot.narrationState(), 0L);

        List<HudRenderLayer> layers = BongHudOrchestrator.buildCommands(snapshot, 250L, FIXED_WIDTH, 220)
            .stream()
            .map(HudRenderCommand::layer)
            .toList();

        assertEquals(List.of(
            HudRenderLayer.BASELINE,
            HudRenderLayer.ZONE,
            HudRenderLayer.TOAST,
            HudRenderLayer.VISUAL
        ), layers);
    }

    @Test
    void localNegativePressureAppearsBelowZoneLine() {
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

        assertTrue(commands.stream().anyMatch(cmd -> "灵压 -0.80".equals(cmd.text())));
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
        HudRenderCommand zoneCommand = commands.get(1);
        HudRenderCommand toastCommand = commands.get(2);

        assertEquals(HudRenderLayer.ZONE, zoneCommand.layer());
        assertEquals(HudRenderLayer.TOAST, toastCommand.layer());
        assertTrue(zoneCommand.text().endsWith("..."));
        assertTrue(toastCommand.isToast());
        assertTrue(toastCommand.text().endsWith("..."));
        assertTrue(FIXED_WIDTH.measure(zoneCommand.text()) <= 72);
        assertTrue(FIXED_WIDTH.measure(toastCommand.text()) <= 72);
        assertEquals(3, commands.size());
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

        assertEquals(2, commands.size());
        assertEquals(HudRenderLayer.BASELINE, commands.get(0).layer());
        assertEquals(HudRenderLayer.TOAST, commands.get(1).layer());
        assertTrue(commands.get(1).text().startsWith("天道警示：") || commands.get(1).text().startsWith("天道警示"));
    }

    @Test
    void overlyNarrowWidthDropsOversizedContentWithoutBreakingBaseline() {
        BongHudStateSnapshot snapshot = BongHudStateSnapshot.create(
            ZoneState.create("jade_valley", "Ancient Jade Valley", 0.8, 2, 100L),
            NarrationState.create("zone", "jade_valley", "Danger rises swiftly.", "system_warning"),
            VisualEffectState.none()
        );

        List<HudRenderCommand> commands = BongHudOrchestrator.buildCommands(snapshot, 0L, FIXED_WIDTH, 2);

        assertEquals(1, commands.size());
        assertEquals(HudRenderLayer.BASELINE, commands.get(0).layer());
    }

    @Test
    void overweightIndicatorAppearsBelowBaselineWhenInventoryExceedsLimit() {
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

        assertEquals(2, commands.size());
        assertEquals(HudRenderLayer.BASELINE, commands.get(0).layer());
        assertEquals(HudRenderLayer.BASELINE, commands.get(1).layer());
        assertTrue(commands.get(1).text().contains("超载"));
    }

    @Test
    void identityHudCornerLabelIsIncludedWhenIdentityStateExists() {
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

        assertTrue(commands.stream().anyMatch(cmd -> "[#0] 白面".equals(cmd.text())));
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
        assertTrue(combatCommands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.STAMINA_BAR));
        assertTrue(combatCommands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.QUICK_BAR));

        HudImmersionMode.resetForTests();
        List<HudRenderCommand> peaceCommands = BongHudOrchestrator.buildCommands(
            BongHudStateSnapshot.empty(), CombatHudSnapshot.empty(), 12_000L, FIXED_WIDTH, 220, 320, 180
        );
        assertTrue(peaceCommands.stream().noneMatch(cmd -> cmd.layer() == HudRenderLayer.STAMINA_BAR));

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
}
