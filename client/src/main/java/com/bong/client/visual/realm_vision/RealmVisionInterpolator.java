package com.bong.client.visual.realm_vision;

public final class RealmVisionInterpolator {
    private RealmVisionInterpolator() {
    }

    public static RealmVisionCommand interpolate(
        RealmVisionCommand from,
        RealmVisionCommand to,
        int transitionTicks,
        int elapsedTicks
    ) {
        if (to == null) {
            return null;
        }
        if (from == null || transitionTicks <= 0 || elapsedTicks >= transitionTicks) {
            return to;
        }
        double t = Math.max(0.0, Math.min(1.0, (double) elapsedTicks / (double) transitionTicks));
        return new RealmVisionCommand(
            lerp(from.fogStart(), to.fogStart(), t),
            lerp(from.fogEnd(), to.fogEnd(), t),
            lerpRgb(from.fogColorRgb(), to.fogColorRgb(), t),
            t < 0.5 ? from.fogShape() : to.fogShape(),
            lerp(from.vignetteAlpha(), to.vignetteAlpha(), t),
            lerpArgb(from.tintColorArgb(), to.tintColorArgb(), t),
            lerp(from.particleDensity(), to.particleDensity(), t),
            lerp(from.postFxSharpen(), to.postFxSharpen(), t)
        );
    }

    private static double lerp(double from, double to, double t) {
        return from + (to - from) * t;
    }

    private static int lerpRgb(int from, int to, double t) {
        return (lerpChannel((from >>> 16) & 0xFF, (to >>> 16) & 0xFF, t) << 16)
            | (lerpChannel((from >>> 8) & 0xFF, (to >>> 8) & 0xFF, t) << 8)
            | lerpChannel(from & 0xFF, to & 0xFF, t);
    }

    private static int lerpArgb(int from, int to, double t) {
        return (lerpChannel((from >>> 24) & 0xFF, (to >>> 24) & 0xFF, t) << 24)
            | (lerpChannel((from >>> 16) & 0xFF, (to >>> 16) & 0xFF, t) << 16)
            | (lerpChannel((from >>> 8) & 0xFF, (to >>> 8) & 0xFF, t) << 8)
            | lerpChannel(from & 0xFF, to & 0xFF, t);
    }

    private static int lerpChannel(int from, int to, double t) {
        return (int) Math.round(lerp(from, to, t));
    }
}
