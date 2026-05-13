package com.bong.client.hud;

import com.bong.client.movement.MovementState;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class MovementHudPlannerTest {
    @Test
    void hidesAfterAutoHideFadeWindow() {
        MovementState state = state(MovementState.Action.NONE, MovementState.ZoneKind.NORMAL, false, 1_000L, 0L);

        assertFalse(MovementHudPlanner.buildCommands(state, 800, 600, 4_200L).isEmpty());
        assertTrue(MovementHudPlanner.buildCommands(state, 800, 600, 4_600L).isEmpty());
    }

    @Test
    void lowStaminaDoesNotKeepDashIndicatorVisible() {
        MovementState state = state(MovementState.Action.NONE, MovementState.ZoneKind.NORMAL, true, 1_000L, 0L);

        List<HudRenderCommand> commands = MovementHudPlanner.buildCommands(state, 800, 600, 20_000L);

        assertTrue(commands.isEmpty());
    }

    @Test
    void rendersOnlyDashCooldownPanelWithoutStaminaBar() {
        MovementState state = new MovementState(
            0.75,
            true,
            MovementState.Action.DASHING,
            MovementState.ZoneKind.NORMAL,
            20,
            1.8,
            70,
            100,
            false,
            12L,
            "",
            1_000L,
            1_000L,
            0L
        );

        List<HudRenderCommand> commands = MovementHudPlanner.buildCommands(state, 800, 600, 1_100L);

        assertEquals(3, commands.stream().filter(MovementHudPlannerTest::isMovementRect).count());
        assertTrue(commands.stream().anyMatch(c -> c.isScaledText() && "DASH".equals(c.text())));
        assertTrue(commands.stream().noneMatch(c -> isMovementRect(c) && c.color() == 0xFFFFD060));
    }

    @Test
    void dashPanelSitsBesideHotbarWithoutHorizontalOverlap() {
        MovementState state = state(MovementState.Action.DASHING, MovementState.ZoneKind.NORMAL, false, 1_000L, 0L);

        List<HudRenderCommand> commands = MovementHudPlanner.buildCommands(state, 398, 121, 1_100L);

        HudRenderCommand panel = commands.stream()
            .filter(c -> isMovementRect(c)
                && c.width() == MovementHudPlanner.PANEL_WIDTH
                && c.height() == MovementHudPlanner.PANEL_HEIGHT)
            .findFirst()
            .orElseThrow();
        int hotbarWidth = QuickBarHudPlanner.TOTAL_SLOTS * QuickBarHudPlanner.SLOT_SIZE
            + (QuickBarHudPlanner.TOTAL_SLOTS - 1) * QuickBarHudPlanner.SLOT_GAP;
        int hotbarLeftX = (398 - hotbarWidth) / 2;
        int hotbarRightX = hotbarLeftX + hotbarWidth;

        assertTrue(panel.x() >= hotbarRightX || panel.x() + panel.width() <= hotbarLeftX);
    }

    @Test
    void deadZoneAddsVignetteEvenWithoutRecentAction() {
        MovementState state = state(MovementState.Action.NONE, MovementState.ZoneKind.DEAD, false, 0L, 0L);

        List<HudRenderCommand> commands = MovementHudPlanner.buildCommands(state, 800, 600, 10_000L);

        assertEquals(1, commands.stream().filter(HudRenderCommand::isEdgeVignette).count());
    }

    @Test
    void rejectedActionAddsRedFlashRect() {
        MovementState state = state(MovementState.Action.NONE, MovementState.ZoneKind.NORMAL, false, 1_000L, 1_000L)
            .withTiming(1_000L, 1_000L, 1_000L);

        List<HudRenderCommand> commands = MovementHudPlanner.buildCommands(state, 800, 600, 1_100L);

        assertTrue(commands.stream().anyMatch(c -> isMovementRect(c) && c.color() == 0xC0FF3030));
    }

    private static boolean isMovementRect(HudRenderCommand command) {
        return command.layer() == HudRenderLayer.MOVEMENT_HUD && command.isRect();
    }

    private static MovementState state(
        MovementState.Action action,
        MovementState.ZoneKind zone,
        boolean lowStamina,
        long hudActivityAtMs,
        long rejectedAtMs
    ) {
        return new MovementState(
            0.75,
            false,
            action,
            zone,
            0,
            1.8,
            lowStamina ? 20 : 80,
            100,
            lowStamina,
            hudActivityAtMs > 0 ? 10L : null,
            rejectedAtMs > 0 ? "dash" : "",
            hudActivityAtMs,
            hudActivityAtMs,
            rejectedAtMs
        );
    }
}
