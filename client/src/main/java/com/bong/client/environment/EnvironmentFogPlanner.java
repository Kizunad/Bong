package com.bong.client.environment;

import net.minecraft.util.math.Vec3d;

import java.util.Collection;
import java.util.Comparator;

public final class EnvironmentFogPlanner {
    private static final int BASE_SKY_RGB = 0x94A9BC;

    private EnvironmentFogPlanner() {
    }

    public static EnvironmentFogCommand plan(Collection<ActiveEmitter> activeEmitters, Vec3d playerPos) {
        if (activeEmitters == null || activeEmitters.isEmpty() || playerPos == null) {
            return null;
        }

        return activeEmitters.stream()
            .filter(emitter -> emitter.effect() instanceof EnvironmentEffect.FogVeil)
            .filter(emitter -> emitter.effect().contains(playerPos))
            .max(Comparator
                .comparingLong(ActiveEmitter::generation)
                .thenComparingDouble(ActiveEmitter::alpha))
            .map(EnvironmentFogPlanner::toCommand)
            .orElse(null);
    }

    private static EnvironmentFogCommand toCommand(ActiveEmitter emitter) {
        EnvironmentEffect.FogVeil fog = (EnvironmentEffect.FogVeil) emitter.effect();
        double density = clamp01(fog.density() * emitter.alpha());
        double fogStart = 28.0 - density * 18.0;
        double fogEnd = 96.0 - density * 52.0;
        int skyColorRgb = blendRgb(BASE_SKY_RGB, fog.tintRgb(), Math.min(0.45, density * 0.6));
        return new EnvironmentFogCommand(fogStart, fogEnd, fog.tintRgb(), skyColorRgb, density);
    }

    private static int blendRgb(int from, int to, double t) {
        double weight = clamp01(t);
        int r = blendChannel((from >>> 16) & 0xFF, (to >>> 16) & 0xFF, weight);
        int g = blendChannel((from >>> 8) & 0xFF, (to >>> 8) & 0xFF, weight);
        int b = blendChannel(from & 0xFF, to & 0xFF, weight);
        return (r << 16) | (g << 8) | b;
    }

    private static int blendChannel(int from, int to, double t) {
        return (int) Math.round(from + (to - from) * t);
    }

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }
}
