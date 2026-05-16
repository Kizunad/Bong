package com.bong.client.movement;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class MovementKeybindingsTest {
    @Test
    void dashYawUsesCameraPovWhenAvailable() {
        assertEquals(
            135.0,
            MovementKeybindings.resolveDashYawDegrees(135.0, 20.0)
        );
    }

    @Test
    void dashYawFallsBackToPlayerYawWhenCameraYawIsMissingOrInvalid() {
        assertEquals(
            -45.0,
            MovementKeybindings.resolveDashYawDegrees(null, -45.0)
        );
        assertEquals(
            90.0,
            MovementKeybindings.resolveDashYawDegrees(Double.NaN, 90.0)
        );
    }
}
