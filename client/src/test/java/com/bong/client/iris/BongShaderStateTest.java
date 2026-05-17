package com.bong.client.iris;

import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class BongShaderStateTest {
    @BeforeEach
    void setUp() {
        BongShaderState.reset();
    }

    @Test
    void initialValuesAreZero() {
        for (BongUniform u : BongUniform.values()) {
            assertEquals(0f, BongShaderState.get(u), 0.0001f,
                    "Initial value of " + u.shaderName() + " should be 0");
            assertEquals(0f, BongShaderState.getTarget(u), 0.0001f,
                    "Initial target of " + u.shaderName() + " should be 0");
        }
    }

    @Test
    void setTargetClampsToZeroOne() {
        BongShaderState.setTarget(BongUniform.BLOODMOON, 0.5f);
        assertEquals(0.5f, BongShaderState.getTarget(BongUniform.BLOODMOON), 0.0001f);

        BongShaderState.setTarget(BongUniform.BLOODMOON, 2.0f);
        assertEquals(1.0f, BongShaderState.getTarget(BongUniform.BLOODMOON), 0.0001f,
                "Values above 1.0 should be clamped to 1.0");

        BongShaderState.setTarget(BongUniform.BLOODMOON, -1.0f);
        assertEquals(0f, BongShaderState.getTarget(BongUniform.BLOODMOON), 0.0001f,
                "Values below 0 should be clamped to 0");
    }

    @Test
    void tickInterpolateMovesCurrent() {
        BongShaderState.setTarget(BongUniform.INKWASH, 1.0f);
        assertEquals(0f, BongShaderState.get(BongUniform.INKWASH), 0.0001f,
                "Before tick, current should still be 0");

        BongShaderState.tickInterpolate();
        float afterOneTick = BongShaderState.get(BongUniform.INKWASH);
        assertTrue(afterOneTick > 0f, "After one tick, current should have moved toward target");
        assertTrue(afterOneTick < 1.0f, "After one tick, current should not have reached target yet");
    }

    @Test
    void tickInterpolateConverges() {
        BongShaderState.setTarget(BongUniform.REALM, 1.0f);
        for (int i = 0; i < 200; i++) {
            BongShaderState.tickInterpolate();
        }
        assertEquals(1.0f, BongShaderState.get(BongUniform.REALM), 0.001f,
                "After many ticks, current should converge to target");
    }

    @Test
    void overrideBypassesInterpolation() {
        BongShaderState.setOverride(BongUniform.TRIBULATION, 0.8f);
        assertEquals(0.8f, BongShaderState.get(BongUniform.TRIBULATION), 0.0001f,
                "Override should set current immediately");
        assertTrue(BongShaderState.isOverridden(BongUniform.TRIBULATION));

        BongShaderState.setTarget(BongUniform.TRIBULATION, 0.2f);
        BongShaderState.tickInterpolate();
        assertEquals(0.8f, BongShaderState.get(BongUniform.TRIBULATION), 0.0001f,
                "Overridden uniform should not move during tick");
    }

    @Test
    void clearOverrideRestoresInterpolation() {
        BongShaderState.setOverride(BongUniform.MEDITATION, 1.0f);
        BongShaderState.clearOverride(BongUniform.MEDITATION);
        assertFalse(BongShaderState.isOverridden(BongUniform.MEDITATION));

        BongShaderState.setTarget(BongUniform.MEDITATION, 0.0f);
        BongShaderState.tickInterpolate();
        float after = BongShaderState.get(BongUniform.MEDITATION);
        assertTrue(after < 1.0f, "After clearing override, interpolation should resume toward target");
    }

    @Test
    void clearAllOverrides() {
        for (BongUniform u : BongUniform.values()) {
            BongShaderState.setOverride(u, 0.5f);
        }
        BongShaderState.clearAllOverrides();
        for (BongUniform u : BongUniform.values()) {
            assertFalse(BongShaderState.isOverridden(u),
                    u.shaderName() + " should not be overridden after clearAll");
        }
    }

    @Test
    void resetClearsEverything() {
        BongShaderState.setTarget(BongUniform.DEMONIC, 1.0f);
        BongShaderState.setOverride(BongUniform.ENLIGHTENMENT, 0.9f);
        for (int i = 0; i < 10; i++) {
            BongShaderState.tickInterpolate();
        }

        BongShaderState.reset();
        for (BongUniform u : BongUniform.values()) {
            assertEquals(0f, BongShaderState.get(u), 0.0001f,
                    "After reset, " + u.shaderName() + " current should be 0");
            assertEquals(0f, BongShaderState.getTarget(u), 0.0001f,
                    "After reset, " + u.shaderName() + " target should be 0");
            assertFalse(BongShaderState.isOverridden(u),
                    "After reset, " + u.shaderName() + " should not be overridden");
        }
    }

    @Test
    void windAngleAllowsRangeUpToTwoPi() {
        float twoPi = (float) (Math.PI * 2);
        BongShaderState.setOverride(BongUniform.WIND_ANGLE, twoPi);
        assertEquals(twoPi, BongShaderState.get(BongUniform.WIND_ANGLE), 0.001f,
                "WIND_ANGLE should allow values up to 2*PI");

        BongShaderState.setOverride(BongUniform.WIND_ANGLE, 10.0f);
        assertEquals(twoPi, BongShaderState.get(BongUniform.WIND_ANGLE), 0.001f,
                "WIND_ANGLE should be clamped to 2*PI max");
    }

    @Test
    void customLerpSpeedAffectsConvergence() {
        BongShaderState.setLerpSpeed(BongUniform.LINGQI, 1.0f);
        BongShaderState.setTarget(BongUniform.LINGQI, 1.0f);
        BongShaderState.tickInterpolate();
        assertEquals(1.0f, BongShaderState.get(BongUniform.LINGQI), 0.001f,
                "With lerp speed 1.0, should reach target in one tick");
    }

    @Test
    void lerpSpeedClampedAtLowerBound() {
        BongShaderState.setLerpSpeed(BongUniform.LINGQI, 0.0f);
        BongShaderState.setTarget(BongUniform.LINGQI, 1.0f);
        BongShaderState.tickInterpolate();
        float after = BongShaderState.get(BongUniform.LINGQI);
        assertTrue(after > 0f, "Even with speed=0 (clamped to 0.001), value should still move slightly");
        assertTrue(after < 0.01f, "With minimum clamped speed, movement should be minimal");
    }

    @Test
    void lerpSpeedClampedAtUpperBound() {
        BongShaderState.setLerpSpeed(BongUniform.LINGQI, 1.5f);
        BongShaderState.setTarget(BongUniform.LINGQI, 1.0f);
        BongShaderState.tickInterpolate();
        assertEquals(1.0f, BongShaderState.get(BongUniform.LINGQI), 0.001f,
                "Speed > 1.0 should be clamped to 1.0, reaching target in one tick");
    }
}
