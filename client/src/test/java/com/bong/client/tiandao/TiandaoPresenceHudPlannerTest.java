package com.bong.client.tiandao;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
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
    void pressureStateAddsTintAndShakeButNoSkyCracks() {
        TiandaoPresenceState state = new TiandaoPresenceState(
            true,
            "pressure",
            48.0,
            "spawn",
            0.4,
            0x400800,
            0.08,
            0.35,
            0.95,
            10L
        );
        List<HudRenderCommand> cmds = TiandaoPresenceHudPlanner.buildCommands(state, 1000L, 1920, 1080);

        assertTrue(cmds.stream().anyMatch(HudRenderCommand::isEdgeVignette));
        assertTrue(cmds.stream().anyMatch(HudRenderCommand::isScreenTint));
        assertEquals(0, skyCrackRectCount(cmds));
        assertTrue(cmds.stream()
            .filter(command -> command.isRect() && command.layer() == HudRenderLayer.EDGE_FEEDBACK)
            .count() >= 2);
    }

    @Test
    void tribulationStateAddsSkyCrackOverlay() {
        TiandaoPresenceState state = new TiandaoPresenceState(
            true,
            "tribulation",
            75.0,
            "spawn",
            0.2,
            0x600000,
            0.15,
            0.65,
            0.85,
            10L
        );
        List<HudRenderCommand> cmds = TiandaoPresenceHudPlanner.buildCommands(state, 1000L, 1920, 1080);

        assertTrue(cmds.stream().anyMatch(HudRenderCommand::isEdgeVignette));
        assertTrue(cmds.stream().anyMatch(HudRenderCommand::isScreenTint));
        assertEquals(12, skyCrackRectCount(cmds));
    }

    @Test
    void annihilateStateDeepensSkyCrackOverlay() {
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
        assertEquals(20, skyCrackRectCount(cmds));
        assertTrue(cmds.stream()
            .anyMatch(command -> command.isRect()
                && command.layer() == HudRenderLayer.VISUAL
                && command.height() == 2));
    }

    private static long skyCrackRectCount(List<HudRenderCommand> commands) {
        return commands.stream()
            .filter(command -> command.isRect() && command.layer() == HudRenderLayer.VISUAL)
            .count();
    }
}
