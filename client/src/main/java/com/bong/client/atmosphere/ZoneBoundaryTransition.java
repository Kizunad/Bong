package com.bong.client.atmosphere;

import java.util.ArrayList;
import java.util.List;

public final class ZoneBoundaryTransition {
    public static final double WIDTH_BLOCKS = 150.0;

    private ZoneBoundaryTransition() {
    }

    public static double progress(double distanceIntoTransitionBlocks) {
        return ZoneAtmosphereProfile.clamp01(distanceIntoTransitionBlocks / WIDTH_BLOCKS);
    }

    public static ZoneAtmosphereProfile blend(
        ZoneAtmosphereProfile from,
        ZoneAtmosphereProfile to,
        double transitionProgress
    ) {
        if (from == null) {
            return to;
        }
        if (to == null) {
            return from;
        }
        double t = ZoneAtmosphereProfile.clamp01(transitionProgress);
        List<ZoneAtmosphereProfile.ParticleConfig> particles = new ArrayList<>();
        from.ambientParticles().stream().map(p -> p.scaled(1.0 - t)).forEach(particles::add);
        to.ambientParticles().stream().map(p -> p.scaled(t)).forEach(particles::add);
        return new ZoneAtmosphereProfile(
            from.zoneId() + "->" + to.zoneId(),
            blendRgb(from.fogColorRgb(), to.fogColorRgb(), t),
            lerp(from.fogDensity(), to.fogDensity(), t),
            particles,
            blendRgb(from.skyTintRgb(), to.skyTintRgb(), t),
            t < 0.5 ? from.entryTransitionFx() : to.entryTransitionFx(),
            t < 0.5 ? from.ambientRecipeId() : to.ambientRecipeId()
        );
    }

    public static int blendRgb(int from, int to, double t) {
        double weight = ZoneAtmosphereProfile.clamp01(t);
        int r = blendChannel((from >>> 16) & 0xFF, (to >>> 16) & 0xFF, weight);
        int g = blendChannel((from >>> 8) & 0xFF, (to >>> 8) & 0xFF, weight);
        int b = blendChannel(from & 0xFF, to & 0xFF, weight);
        return (r << 16) | (g << 8) | b;
    }

    public static double lerp(double from, double to, double t) {
        double weight = ZoneAtmosphereProfile.clamp01(t);
        return from + (to - from) * weight;
    }

    private static int blendChannel(int from, int to, double t) {
        return (int) Math.round(from + (to - from) * t);
    }
}
