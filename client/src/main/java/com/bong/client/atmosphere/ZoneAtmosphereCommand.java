package com.bong.client.atmosphere;

import java.util.List;

public record ZoneAtmosphereCommand(
    String zoneId,
    int fogColorRgb,
    double fogDensity,
    double fogStart,
    double fogEnd,
    int skyTintRgb,
    List<ZoneAtmosphereProfile.ParticleConfig> particles,
    ZoneAtmosphereProfile.TransitionFx entryTransitionFx,
    String ambientRecipeId,
    double desaturation,
    double vignetteIntensity,
    int vignetteColorRgb,
    double distortionIntensity,
    double breathingScale,
    double cameraShakeIntensity,
    boolean hardClipVoid,
    boolean deadZoneVisual,
    boolean negativeZoneVisual
) {
    public ZoneAtmosphereCommand {
        zoneId = ZoneAtmosphereProfile.normalizeId(zoneId, "wilderness");
        fogColorRgb &= 0x00FFFFFF;
        fogDensity = ZoneAtmosphereProfile.clamp01(fogDensity);
        fogStart = Math.max(0.0, finiteOrZero(fogStart));
        fogEnd = Math.max(fogStart, finiteOrZero(fogEnd));
        skyTintRgb &= 0x00FFFFFF;
        particles = List.copyOf(particles == null ? List.of() : particles);
        entryTransitionFx = entryTransitionFx == null ? ZoneAtmosphereProfile.TransitionFx.NONE : entryTransitionFx;
        ambientRecipeId = ZoneAtmosphereProfile.normalizeId(ambientRecipeId, "");
        desaturation = ZoneAtmosphereProfile.clamp01(desaturation);
        vignetteIntensity = ZoneAtmosphereProfile.clamp01(vignetteIntensity);
        vignetteColorRgb &= 0x00FFFFFF;
        distortionIntensity = ZoneAtmosphereProfile.clamp01(distortionIntensity);
        breathingScale = Math.max(0.0, Math.min(0.05, finiteOrZero(breathingScale)));
        cameraShakeIntensity = ZoneAtmosphereProfile.clamp01(cameraShakeIntensity);
    }

    public int fogColorArgb(int alpha) {
        return (clampAlpha(alpha) << 24) | fogColorRgb;
    }

    public int skyTintArgb(int alpha) {
        return (clampAlpha(alpha) << 24) | skyTintRgb;
    }

    public int vignetteArgb() {
        return (clampAlpha((int) Math.round(180.0 * vignetteIntensity)) << 24) | vignetteColorRgb;
    }

    public double estimatedFrameCostMs() {
        double particleDensity = particles.stream().mapToDouble(ZoneAtmosphereProfile.ParticleConfig::density).sum();
        return 0.12 + fogDensity * 0.28 + particleDensity * 0.32 + distortionIntensity * 0.35 + cameraShakeIntensity * 0.18;
    }

    private static int clampAlpha(int alpha) {
        return Math.max(0, Math.min(255, alpha));
    }

    private static double finiteOrZero(double value) {
        return Double.isFinite(value) ? value : 0.0;
    }
}
