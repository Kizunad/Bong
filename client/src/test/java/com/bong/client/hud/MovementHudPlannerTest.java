package com.bong.client.hud;

import com.bong.client.movement.MovementState;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class MovementHudPlannerTest {
    @Test
    void unknownDashHidesEmblemButPreservesZoneFeedback() {
        var normal = state(0, 0, MovementState.ZoneKind.NORMAL);
        assertTrue(MovementHudPlanner.buildCommands(normal, false, 800, 600, 1_000).isEmpty());
        var dead = state(0, 0, MovementState.ZoneKind.DEAD);
        var commands = MovementHudPlanner.buildCommands(dead, false, 800, 600, 1_000);
        assertTrue(commands.stream().anyMatch(HudRenderCommand::isEdgeVignette));
        assertFalse(commands.stream().anyMatch(HudRenderCommand::isTexturedRect));
    }

    @Test
    void progressUsesAuthoritativeDurationAndWaitsForReadySnapshot() {
        var cooling = state(10, 20, MovementState.ZoneKind.NORMAL);
        assertEquals(.5, MovementHudPlanner.readyFraction(cooling, 1_000), .001);
        assertEquals(.75, MovementHudPlanner.readyFraction(cooling, 1_250), .001);
        assertTrue(MovementHudPlanner.readyFraction(cooling, 10_000) < 1,
            "延迟快照不能提前宣布身法可用");
        assertEquals(1, MovementHudPlanner.readyFraction(state(0, 20, MovementState.ZoneKind.NORMAL), 10_000));
        assertEquals(0, MovementHudPlanner.readyFraction(state(10, 0, MovementState.ZoneKind.NORMAL), 1_000));
    }

    @Test
    void learnedDashStaysVisibleAndFitsAlongsideQuickbar() {
        var ready = state(0, 0, MovementState.ZoneKind.NORMAL);
        for (int width : new int[]{640, 320, 166}) {
            int height = 180;
            var commands = MovementHudPlanner.buildCommands(ready, true, width, height, 50_000);
            var icon = commands.stream().filter(HudRenderCommand::isTexturedRect).findFirst().orElseThrow();
            assertEquals(MovementHudPlanner.ICON, icon.texturePath());
            assertEquals(icon.width(), icon.height(), "身法图不能拉伸");
            for (var command : commands) {
                assertTrue(command.x() >= 0 && command.y() >= 0
                    && command.x() + command.width() <= width
                    && command.y() + command.height() <= height, "窄窗口也不能越界");
            }
            int hotbarLeft = QuickBarHudPlanner.rowLeftX(width, QuickBarHudPlanner.TOTAL_SLOTS);
            int upperY = height - QuickBarHudPlanner.LOWER_BOTTOM_MARGIN
                - QuickBarHudPlanner.SLOT_SIZE * 2 - QuickBarHudPlanner.UPPER_GAP;
            assertTrue(icon.x() + icon.width() <= hotbarLeft
                || icon.x() >= width - hotbarLeft || icon.y() + icon.height() <= upperY,
                "身法图必须避开居中双排槽位");
        }
    }

    private static MovementState state(long remaining, long total, MovementState.ZoneKind zone) {
        return new MovementState(.75, false, MovementState.Action.NONE, zone, remaining, total,
            1.8, 80, 100, false, null, "", 1_000, 0, 0);
    }
}
