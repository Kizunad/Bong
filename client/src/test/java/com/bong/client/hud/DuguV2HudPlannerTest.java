package com.bong.client.hud;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertTrue;

class DuguV2HudPlannerTest {
    @Test
    void activeStateShowsTaintRevealSelfCureAndShroud() {
        DuguV2HudStateStore.State state = new DuguV2HudStateStore.State(
            true,
            0.7f,
            "蛊毒入髓",
            0.45f,
            62.5f,
            true,
            true,
            10_000L
        );

        List<HudRenderCommand> commands = DuguV2HudPlanner.buildCommands(state, 960, 540, 1_000L);

        assertTrue(commands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.DUGU_TAINT_WARNING && cmd.isEdgeVignette()));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.DUGU_TAINT_INDICATOR && cmd.isText()));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.DUGU_REVEAL_RISK && cmd.isRect()));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.DUGU_SELF_CURE_PROGRESS && cmd.isText()));
        assertTrue(commands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.DUGU_SHROUD && cmd.isScreenTint()));
    }

    @Test
    void emptyStateDoesNotEmitDuguHud() {
        assertTrue(DuguV2HudPlanner.buildCommands(DuguV2HudStateStore.State.NONE, 960, 540, 1_000L).isEmpty());
    }
}
