package com.bong.client.visual.realm_vision;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class RealmVisionInterpolatorTest {
    @Test
    void interpolatesMiddleFrame() {
        RealmVisionCommand from = new RealmVisionCommand(30.0, 60.0, 0x000000, FogShape.CYLINDER, 0.5, 0, 0.0, 0.0);
        RealmVisionCommand to = new RealmVisionCommand(130.0, 160.0, 0xFFFFFF, FogShape.SPHERE, 0.0, 0x10FFFFFF, 1.0, 0.8);
        RealmVisionCommand mid = RealmVisionInterpolator.interpolate(from, to, 100, 50);
        assertEquals(80.0, mid.fogStart());
        assertEquals(110.0, mid.fogEnd());
        assertEquals(0x808080, mid.fogColorRgb());
        assertEquals(FogShape.SPHERE, mid.fogShape());
        assertEquals(0.25, mid.vignetteAlpha());
        assertEquals(0.5, mid.particleDensity());
        assertEquals(0.4, mid.postFxSharpen());
    }

    @Test
    void returnsTargetWhenFinished() {
        RealmVisionCommand to = new RealmVisionCommand(30.0, 60.0, 0xB8B0A8, FogShape.CYLINDER, 0.55, 0, 0.0, 0.0);
        assertEquals(to, RealmVisionInterpolator.interpolate(null, to, 0, 0));
    }
}
