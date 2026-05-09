package com.bong.client.hud;

import com.bong.client.combat.store.VortexStateStore;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertTrue;

class WoliuV2HudPlannerTest {
    @Test
    void activeVortexShowsChargeCooldownBackfireAndTurbulence() {
        VortexStateStore.State state = new VortexStateStore.State(
            true,
            5f,
            0.8f,
            0.9f,
            80L,
            2,
            "woliu.heart",
            0.65f,
            8_000L,
            "severed",
            30f,
            0.75f,
            10_000L
        );

        List<HudRenderCommand> commands = WoliuV2HudPlanner.buildCommands(state, 960, 540, 1_000L);

        assertTrue(commands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.VORTEX_CHARGE && cmd.isRect()));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.VORTEX_COOLDOWN && cmd.isText()));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.VORTEX_BACKFIRE && cmd.isEdgeVignette()));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.VORTEX_TURBULENCE && cmd.isScreenTint()));
    }

    @Test
    void emptyStateDoesNotEmitVortexHud() {
        assertTrue(WoliuV2HudPlanner.buildCommands(VortexStateStore.State.NONE, 960, 540, 1_000L).isEmpty());
    }
}
