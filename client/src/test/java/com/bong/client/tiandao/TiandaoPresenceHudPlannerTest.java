package com.bong.client.tiandao;

import com.bong.client.hud.HudRenderCommand;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class TiandaoPresenceHudPlannerTest {

    @Test
    void inactiveStateProducesNoCommands() {
        List<HudRenderCommand> cmds = TiandaoPresenceHudPlanner.buildCommands(
            TiandaoPresenceState.empty(),
            0L,
            1920,
            1080
        );

        assertTrue(cmds.isEmpty());
    }

    @Test
    void watchStateProducesEdgeVignetteOnly() {
        TiandaoPresenceState state = new TiandaoPresenceState(
            true,
            "watch",
            20.0,
            "spawn",
            0.5,
            0x400800,
            0.03,
            0.0,
            1.0,
            10L
        );
        List<HudRenderCommand> cmds = TiandaoPresenceHudPlanner.buildCommands(state, 1000L, 1920, 1080);

        assertEquals(1, cmds.size());
        assertTrue(cmds.get(0).isEdgeVignette());
    }

    @Test
    void annihilateStateAddsShakeAndTint() {
        TiandaoPresenceState state = new TiandaoPresenceState(
            true,
            "annihilate",
            95.0,
            "spawn",
            0.1,
            0x801000,
            0.25,
            1.0,
            0.7,
            10L
        );
        List<HudRenderCommand> cmds = TiandaoPresenceHudPlanner.buildCommands(state, 1000L, 1920, 1080);

        assertTrue(cmds.stream().anyMatch(HudRenderCommand::isEdgeVignette));
        assertTrue(cmds.stream().anyMatch(HudRenderCommand::isScreenTint));
        assertTrue(cmds.stream().anyMatch(HudRenderCommand::isRect));
    }
}
