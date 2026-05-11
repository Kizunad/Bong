package com.bong.client.movement;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class MovementStateTest {
    @Test
    void emptyStateMatchesResetSnapshotOnly() {
        assertTrue(MovementState.empty().isEmpty());
    }

    @Test
    void passiveSpeedAndStaminaStateIsNotEmpty() {
        MovementState state = new MovementState(
            0.75,
            false,
            MovementState.Action.NONE,
            MovementState.ZoneKind.NORMAL,
            0,
            0,
            0,
            0,
            1.8,
            80,
            100,
            false,
            null,
            "",
            1_000L,
            0L,
            0L
        );

        assertFalse(state.isEmpty());
    }
}
