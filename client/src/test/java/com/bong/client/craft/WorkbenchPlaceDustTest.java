package com.bong.client.craft;

import net.minecraft.util.math.BlockPos;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class WorkbenchPlaceDustTest {
    @Test
    void planSpawnsFourGroundDustParticlesAroundBlockBase() {
        BlockPos pos = new BlockPos(10, 64, -3);
        List<WorkbenchPlaceDust.ParticleSpec> specs = WorkbenchPlaceDust.plan(pos);

        assertEquals(4, specs.size(), "workbench placement should spawn a 4-particle dust burst");
        for (WorkbenchPlaceDust.ParticleSpec spec : specs) {
            assertTrue(spec.x() >= 10.0 && spec.x() <= 11.0, "x should stay inside target block footprint");
            assertEquals(64.1, spec.y(), 1e-5, "dust should sit near the workbench base");
            assertTrue(spec.z() >= -3.0 && spec.z() <= -2.0, "z should stay inside target block footprint");
            assertTrue(Math.abs(spec.vx()) <= 0.05, "horizontal velocity must stay subtle");
            assertTrue(spec.vy() < 0.0, "dust should settle downward");
            assertTrue(Math.abs(spec.vz()) <= 0.05, "horizontal velocity must stay subtle");
        }
    }

    @Test
    void constantsMatchPlanVisualSpec() {
        assertEquals(0x8B7355, WorkbenchPlaceDust.RGB);
        assertEquals(12, WorkbenchPlaceDust.LIFETIME_TICKS);
        assertEquals(0.65f, WorkbenchPlaceDust.ALPHA, 1e-5f);
        assertEquals(0.12f, WorkbenchPlaceDust.SCALE, 1e-5f);
    }
}
