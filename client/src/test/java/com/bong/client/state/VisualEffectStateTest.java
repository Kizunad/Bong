package com.bong.client.state;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class VisualEffectStateTest {
    @Test
    void noneFactoryReturnsInactiveEffect() {
        VisualEffectState state = VisualEffectState.none();

        assertTrue(state.isEmpty());
        assertFalse(state.isActiveAt(0L));
        assertEquals(0.0, state.remainingRatioAt(0L));
        assertEquals(0.0, state.scaledIntensityAt(0L));
    }

    @Test
    void createClampsIntensityAndComputesDecay() {
        VisualEffectState state = VisualEffectState.create("camera_shake", 2.0, 1_000L, 100L);

        assertEquals(VisualEffectState.EffectType.SCREEN_SHAKE, state.effectType());
        assertEquals(1.0, state.intensity(), 0.0001);
        assertTrue(state.isActiveAt(600L));
        assertEquals(0.5, state.remainingRatioAt(600L), 0.0001);
        assertEquals(0.5, state.scaledIntensityAt(600L), 0.0001);
        assertFalse(state.isActiveAt(1_100L));
    }

    @Test
    void unknownOrZeroEffectSafelyBecomesNoOp() {
        VisualEffectState unknown = VisualEffectState.create("unknown_effect", 0.8, 500L, 0L);
        VisualEffectState zeroDuration = VisualEffectState.create("fog_pulse", 0.5, 0L, 0L);

        assertTrue(unknown.isEmpty());
        assertTrue(zeroDuration.isEmpty());
        assertEquals(0.0, zeroDuration.scaledIntensityAt(250L), 0.0001);
    }
}
