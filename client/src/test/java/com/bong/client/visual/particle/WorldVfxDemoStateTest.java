package com.bong.client.visual.particle;

import net.minecraft.util.math.Vec3d;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class WorldVfxDemoStateTest {
    @Test
    void flyingSwordDirectionFallsBackWhenZeroOrInvalid() {
        assertEquals(new Vec3d(1.0, 0.0, 0.0), FlyingSwordDemoState.normalizeDirection(Vec3d.ZERO));
        assertEquals(
            new Vec3d(1.0, 0.0, 0.0),
            FlyingSwordDemoState.normalizeDirection(new Vec3d(Double.NaN, 0.0, 0.0))
        );
    }

    @Test
    void flyingSwordPositionAdvancesAlongNormalizedDirection() {
        FlyingSwordDemoState state = new FlyingSwordDemoState(
            Vec3d.ZERO,
            new Vec3d(10.0, 0.0, 0.0),
            80,
            1.0,
            0x88CCFF
        );

        state.tick();
        Vec3d pos = state.position(0.0f);
        assertTrue(pos.x > 0.0, "demo sword should move forward along normalized direction");
        assertEquals(0.0, pos.z, 1.0e-6);
    }

    @Test
    void formationPulseCadenceIsEverySixteenTicks() {
        FormationCoreDemoState state = new FormationCoreDemoState(Vec3d.ZERO, 120, 0.9, 0xC4E0FF);
        assertTrue(state.shouldPulse(), "new formation demo should emit an initial pulse");

        state.tick();
        assertFalse(state.shouldPulse(), "age 1 should not double-emit the initial pulse");

        for (int i = 1; i < 15; i++) {
            state.tick();
        }
        assertTrue(!state.shouldPulse(), "age 15 is not a pulse frame");

        state.tick();
        assertTrue(state.shouldPulse(), "age 16 should pulse again");
    }
}
