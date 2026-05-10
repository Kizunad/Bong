package com.bong.client.hud;

import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.DerivedAttrFlags;
import com.bong.client.state.VisualEffectState;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class HudImmersionModeTest {
    @AfterEach
    void reset() {
        HudImmersionMode.resetForTests();
    }

    @Test
    void combatGraceWindowDoesNotSurviveBackwardClockJump() {
        CombatHudState combat = CombatHudState.create(0.8f, 0.7f, 0.4f, DerivedAttrFlags.none());

        assertEquals(
            HudImmersionMode.Mode.COMBAT,
            HudImmersionMode.resolve(combat, VisualEffectState.none(), 1_000L)
        );
        assertEquals(
            HudImmersionMode.Mode.PEACE,
            HudImmersionMode.resolve(CombatHudState.empty(), VisualEffectState.none(), 900L)
        );
    }

    @Test
    void combatFilterDropsNullCommandsBeforeCopying() {
        List<HudRenderCommand> commands = new ArrayList<>();
        commands.add(HudRenderCommand.rect(HudRenderLayer.QUICK_BAR, 1, 2, 3, 4, 0xFFFFFFFF));
        commands.add(null);

        List<HudRenderCommand> filtered = HudImmersionMode.filter(commands, HudImmersionMode.Mode.COMBAT);

        assertEquals(1, filtered.size());
        assertEquals(HudRenderLayer.QUICK_BAR, filtered.get(0).layer());
    }

    @Test
    void combatWinsOverMeditation() {
        CombatHudState combat = CombatHudState.create(0.8f, 0.7f, 0.4f, DerivedAttrFlags.none());
        VisualEffectState meditation = VisualEffectState.create("meditation_calm", 1.0, 5_000L, 1_000L);

        assertEquals(
            HudImmersionMode.Mode.COMBAT,
            HudImmersionMode.resolve(combat, meditation, 1_500L)
        );
    }

    @Test
    void cultivationFilterHidesQuickBar() {
        List<HudRenderCommand> commands = List.of(
            HudRenderCommand.rect(HudRenderLayer.QUICK_BAR, 1, 2, 3, 4, 0xFFFFFFFF),
            HudRenderCommand.rect(HudRenderLayer.ZONE, 1, 2, 3, 4, 0xFFFFFFFF)
        );

        List<HudRenderCommand> filtered = HudImmersionMode.filter(commands, HudImmersionMode.Mode.CULTIVATION);

        assertEquals(1, filtered.size());
        assertEquals(HudRenderLayer.ZONE, filtered.get(0).layer());
    }

    @Test
    void immersiveFadeDuration() {
        HudImmersionMode.setManualImmersive(true, 1_000L);

        assertEquals(1.0, HudImmersionMode.immersiveAlpha(true, false, 1_000L), 0.0001);
        assertEquals(0.0, HudImmersionMode.immersiveAlpha(true, false, 1_500L), 0.0001);
    }

    @Test
    void altPeekTemporary() {
        HudImmersionMode.setManualImmersive(true, 1_000L);
        List<HudRenderCommand> commands = List.of(
            HudRenderCommand.rect(HudRenderLayer.QI_RADAR, 0, 0, 10, 2, 0xFFFFFFFF)
        );

        List<HudRenderCommand> peeked = HudImmersionMode.applyImmersiveAlpha(
            commands,
            HudImmersionMode.Mode.PEACE,
            VisualEffectState.none(),
            new HudRuntimeContext(0.0, 0.0, 0.0, 0.0, true, List.of()),
            1_600L
        );

        assertEquals(0x99FFFFFF, peeked.get(0).color());
        assertTrue(HudImmersionMode.manualImmersive());
    }

    @Test
    void altPeekThreeSecondsExit() {
        HudImmersionMode.setManualImmersive(true, 1_000L);
        List<HudRenderCommand> commands = List.of(
            HudRenderCommand.rect(HudRenderLayer.QI_RADAR, 0, 0, 10, 2, 0xFFFFFFFF)
        );
        HudRuntimeContext altDown = new HudRuntimeContext(0.0, 0.0, 0.0, 0.0, true, List.of());

        HudImmersionMode.applyImmersiveAlpha(commands, HudImmersionMode.Mode.PEACE, VisualEffectState.none(), altDown, 1_000L);
        HudImmersionMode.applyImmersiveAlpha(commands, HudImmersionMode.Mode.PEACE, VisualEffectState.none(), altDown, 4_100L);

        assertFalse(HudImmersionMode.manualImmersive());
    }

    @Test
    void combatTemporaryRestoreKeepsHudOpaque() {
        HudImmersionMode.setManualImmersive(true, 1_000L);
        List<HudRenderCommand> commands = List.of(
            HudRenderCommand.rect(HudRenderLayer.QI_RADAR, 0, 0, 10, 2, 0xFFFFFFFF)
        );

        List<HudRenderCommand> restored = HudImmersionMode.applyImmersiveAlpha(
            commands,
            HudImmersionMode.Mode.COMBAT,
            VisualEffectState.none(),
            HudRuntimeContext.empty(),
            1_600L
        );

        assertEquals(0xFFFFFFFF, restored.get(0).color());
    }

    @Test
    void meditateAutoImmersive() {
        VisualEffectState meditation = VisualEffectState.create("meditation_calm", 1.0, 10_000L, 1_000L);
        List<HudRenderCommand> commands = List.of(
            HudRenderCommand.rect(HudRenderLayer.QI_RADAR, 0, 0, 10, 2, 0xFFFFFFFF)
        );

        HudImmersionMode.applyImmersiveAlpha(commands, HudImmersionMode.Mode.CULTIVATION, meditation, HudRuntimeContext.empty(), 1_000L);
        List<HudRenderCommand> dimmed = HudImmersionMode.applyImmersiveAlpha(
            commands,
            HudImmersionMode.Mode.CULTIVATION,
            meditation,
            HudRuntimeContext.empty(),
            4_500L
        );

        assertEquals(0x00FFFFFF, dimmed.get(0).color());
    }
}
