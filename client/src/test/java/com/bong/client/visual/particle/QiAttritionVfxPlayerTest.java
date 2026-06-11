package com.bong.client.visual.particle;

import com.bong.client.network.QiAttritionPayload;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class QiAttritionVfxPlayerTest {
    @Test
    void plansThreeToFiveDarkGoldParticles() {
        QiAttritionPayload payload = new QiAttritionPayload(44L, 2.0, new double[] {8.0, 65.0, 9.0});
        List<QiAttritionVfxPlayer.ParticleSpec> specs = QiAttritionVfxPlayer.plan(payload);

        assertTrue(specs.size() >= QiAttritionVfxPlayer.MIN_PARTICLES);
        assertTrue(specs.size() <= QiAttritionVfxPlayer.MAX_PARTICLES);
        for (QiAttritionVfxPlayer.ParticleSpec spec : specs) {
            assertEquals(QiAttritionVfxPlayer.COLOR_RGB, spec.colorRgb());
            assertEquals(QiAttritionVfxPlayer.LIFETIME_TICKS, spec.lifetimeTicks());
            assertEquals(QiAttritionVfxPlayer.UPWARD_DRIFT_METERS, spec.upwardDriftMeters());
            assertTrue(spec.vy() > 0.0, "attrition mote should drift upward");
        }
    }

    @Test
    void plansExactlyMinParticlesAtBoundary() {
        QiAttritionPayload payload = new QiAttritionPayload(0L, 1.0, new double[] {0.0, 64.0, 0.0});

        assertEquals(QiAttritionVfxPlayer.MIN_PARTICLES, QiAttritionVfxPlayer.plan(payload).size());
    }

    @Test
    void plansExactlyMaxParticlesAtBoundary() {
        QiAttritionPayload payload = new QiAttritionPayload(2L, 1.0, new double[] {0.0, 64.0, 0.0});

        assertEquals(QiAttritionVfxPlayer.MAX_PARTICLES, QiAttritionVfxPlayer.plan(payload).size());
    }

    @Test
    void planStaysNearWorldPosition() {
        QiAttritionPayload payload = new QiAttritionPayload(7L, 1.0, new double[] {12.0, 70.0, -4.0});
        List<QiAttritionVfxPlayer.ParticleSpec> specs = QiAttritionVfxPlayer.plan(payload);

        for (QiAttritionVfxPlayer.ParticleSpec spec : specs) {
            assertTrue(Math.abs(spec.x() - 12.0) <= 0.2);
            assertTrue(spec.y() >= 70.0);
            assertTrue(spec.y() <= 70.3);
            assertTrue(Math.abs(spec.z() + 4.0) <= 0.2);
        }
    }
}
