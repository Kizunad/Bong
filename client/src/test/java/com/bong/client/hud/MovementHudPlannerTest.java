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
    void lowStaminaKeepsCompactIndicatorVisible() {
        MovementState state = state(MovementState.Action.NONE, MovementState.ZoneKind.NORMAL, true, 1_000L, 0L);

        List<HudRenderCommand> commands = MovementHudPlanner.buildCommands(state, 800, 600, 20_000L);

        assertTrue(commands.stream().anyMatch(c -> c.layer() == HudRenderLayer.MOVEMENT_HUD && c.isRect()));
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

        assertTrue(commands.stream().anyMatch(c -> c.isRect() && c.color() == 0xC0FF3030));
    }

    @Test
    void doubleJumpDotsReflectMaxCharges() {
        MovementState state = new MovementState(
            0.75,
            false,
            MovementState.Action.DOUBLE_JUMPING,
            MovementState.ZoneKind.NORMAL,
            0,
            0,
            1,
            2,
            1.8,
            80,
            100,
            false,
            12L,
            "",
            1_000L,
            1_000L,
            0L
        );

        List<HudRenderCommand> commands = MovementHudPlanner.buildCommands(state, 800, 600, 1_100L);

        long dotRects = commands.stream()
            .filter(c -> c.isRect() && c.width() == 6 && c.height() == 6)
            .count();
        assertEquals(2, dotRects);
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
            0,
            1,
            1,
            1.8,
            lowStamina ? 20 : 80,
            100,
            lowStamina,
            hudActivityAtMs > 0 ? 10L : null,
            rejectedAtMs > 0 ? "stamina_insufficient" : "",
            hudActivityAtMs,
            hudActivityAtMs,
            rejectedAtMs
        );
    }
}
