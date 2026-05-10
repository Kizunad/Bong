package com.bong.client.atmosphere;

import java.util.List;
import java.util.Objects;

public record ZoneAtmosphereProfile(
    String zoneId,
    int fogColorRgb,
    double fogDensity,
    List<ParticleConfig> ambientParticles,
    int skyTintRgb,
    TransitionFx entryTransitionFx,
    String ambientRecipeId
) {
    public ZoneAtmosphereProfile {
        zoneId = normalizeId(zoneId, "wilderness");
        fogColorRgb &= 0x00FFFFFF;
        fogDensity = clamp01(fogDensity);
        ambientParticles = List.copyOf(ambientParticles == null ? List.of() : ambientParticles);
        skyTintRgb &= 0x00FFFFFF;
        entryTransitionFx = entryTransitionFx == null ? TransitionFx.NONE : entryTransitionFx;
        ambientRecipeId = normalizeId(ambientRecipeId, "");
    }

    public ZoneAtmosphereProfile withFogAndSky(int nextFogColorRgb, double nextFogDensity, int nextSkyTintRgb) {
        return new ZoneAtmosphereProfile(
            zoneId,
            nextFogColorRgb,
            nextFogDensity,
            ambientParticles,
            nextSkyTintRgb,
            entryTransitionFx,
            ambientRecipeId
        );
    }

    public ZoneAtmosphereProfile withParticles(List<ParticleConfig> particles) {
        return new ZoneAtmosphereProfile(
            zoneId,
            fogColorRgb,
            fogDensity,
            particles,
            skyTintRgb,
            entryTransitionFx,
            ambientRecipeId
        );
    }

    static double clamp01(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }

    static String normalizeId(String value, String fallback) {
        String normalized = value == null ? "" : value.trim();
        return normalized.isEmpty() ? Objects.requireNonNull(fallback, "fallback") : normalized;
    }

    public enum TransitionFx {
        NONE,
        FADE,
        MIST_BURST,
        WIND_GUST
    }

    public record ParticleConfig(
        String type,
        int tintRgb,
        double density,
        double driftX,
        double driftY,
        double driftZ,
        int intervalTicks
    ) {
        public ParticleConfig {
            type = normalizeId(type, "cloud256_dust");
            tintRgb &= 0x00FFFFFF;
            density = Math.max(0.0, Double.isFinite(density) ? density : 0.0);
            driftX = finiteOrZero(driftX);
            driftY = finiteOrZero(driftY);
            driftZ = finiteOrZero(driftZ);
            intervalTicks = Math.max(1, intervalTicks);
        }

        public ParticleConfig scaled(double factor) {
            return new ParticleConfig(type, tintRgb, density * Math.max(0.0, factor), driftX, driftY, driftZ, intervalTicks);
        }

        public ParticleConfig withDensity(double nextDensity) {
            return new ParticleConfig(type, tintRgb, nextDensity, driftX, driftY, driftZ, intervalTicks);
        }

        public ParticleConfig withDrift(double nextX, double nextY, double nextZ) {
            return new ParticleConfig(type, tintRgb, density, nextX, nextY, nextZ, intervalTicks);
        }

        private static double finiteOrZero(double value) {
            return Double.isFinite(value) ? value : 0.0;
        }
    }
}
