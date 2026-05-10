package com.bong.client.hud;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class HudLayoutPresetTest {
    @AfterEach
    void reset() {
        HudLayoutPreferenceStore.resetForTests();
        HudImmersionMode.resetForTests();
    }

    @Test
    void presetSwitchesOnCombatState() {
        List<HudRenderCommand> commands = List.of(
            HudRenderCommand.rect(HudRenderLayer.THREAT_INDICATOR, 0, 0, 10, 2, 0xFFFF0000),
            HudRenderCommand.rect(HudRenderLayer.QI_RADAR, 0, 0, 10, 2, 0xFFFFFFFF)
        );

        List<HudRenderCommand> peace = HudLayoutPreset.filter(
            commands,
            HudImmersionMode.Mode.PEACE,
            HudLayoutPreferenceStore.Density.STANDARD,
            1_000L
        );
        List<HudRenderCommand> combat = HudLayoutPreset.filter(
            commands,
            HudImmersionMode.Mode.COMBAT,
            HudLayoutPreferenceStore.Density.STANDARD,
            1_000L
        );

        assertTrue(peace.stream().noneMatch(cmd -> cmd.layer() == HudRenderLayer.THREAT_INDICATOR));
        assertTrue(combat.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.THREAT_INDICATOR));
    }

    @Test
    void presetAnimationStagger() {
        assertEquals(0.0, HudLayoutPreset.alphaForWidget(true, 50L), 0.0001);
        assertEquals(1.0, HudLayoutPreset.alphaForWidget(true, 450L), 0.0001);
        assertEquals(0.0, HudLayoutPreset.alphaForWidget(false, 200L), 0.0001);
    }

    @Test
    void densityOverridesPreset() {
        List<HudRenderCommand> commands = List.of(
            HudRenderCommand.rect(HudRenderLayer.COMPASS, 0, 0, 10, 2, 0xFFFFFFFF),
            HudRenderCommand.rect(HudRenderLayer.EVENT_STREAM, 0, 0, 10, 2, 0xFFFFFFFF)
        );

        List<HudRenderCommand> minimal = HudLayoutPreset.filter(
            commands,
            HudImmersionMode.Mode.PEACE,
            HudLayoutPreferenceStore.Density.MINIMAL,
            1_000L
        );

        assertTrue(minimal.stream().noneMatch(cmd -> cmd.layer() == HudRenderLayer.COMPASS));
        assertTrue(minimal.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.EVENT_STREAM));
    }
}
