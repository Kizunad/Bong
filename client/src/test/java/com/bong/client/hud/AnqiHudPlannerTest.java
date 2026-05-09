package com.bong.client.hud;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class AnqiHudPlannerTest {
    private static final long NOW = 1_000L;

    @Test
    void aimEncloseBuildsFourReticleSegments() {
        List<HudRenderCommand> commands = AnqiHudPlanner.buildCommands(
            AnqiHudState.aim(0.5f, NOW, 500L),
            NOW,
            320,
            180
        );

        assertEquals(4, commands.size());
        assertTrue(commands.stream().allMatch(HudRenderCommand::isRect));
        assertTrue(commands.stream().allMatch(cmd -> cmd.layer() == HudRenderLayer.CARRIER));
    }

    @Test
    void chargeRingBuildsProgressBar() {
        List<HudRenderCommand> commands = AnqiHudPlanner.buildCommands(
            AnqiHudState.charge(0.5f, NOW, 500L),
            NOW,
            320,
            180
        );

        assertEquals(2, commands.size());
        assertTrue(commands.stream().anyMatch(cmd -> cmd.layer() == HudRenderLayer.CAST_BAR
            && cmd.width() == 48
            && cmd.height() == 5));
    }

    @Test
    void echoFractalBuildsCountText() {
        List<HudRenderCommand> commands = AnqiHudPlanner.buildCommands(
            AnqiHudState.echo(3, NOW, 500L),
            NOW,
            320,
            180
        );

        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && "echo 3".equals(cmd.text())));
    }

    @Test
    void abrasionTooltipBuildsContainerLine() {
        List<HudRenderCommand> commands = AnqiHudPlanner.buildCommands(
            AnqiHudState.abrasion("jade_tube", 12.25f, NOW, 500L),
            NOW,
            320,
            240
        );

        assertTrue(commands.stream().anyMatch(cmd -> cmd.isText() && "jade_tube 12.3".equals(cmd.text())));
    }

    @Test
    void expiredStateBuildsNoCommands() {
        List<HudRenderCommand> commands = AnqiHudPlanner.buildCommands(
            AnqiHudState.echo(2, NOW, 1L),
            NOW + 2L,
            320,
            180
        );

        assertTrue(commands.isEmpty());
    }
}
