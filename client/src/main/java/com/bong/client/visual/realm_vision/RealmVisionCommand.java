package com.bong.client.visual.realm_vision;

public record RealmVisionCommand(
    double fogStart,
    double fogEnd,
    int fogColorRgb,
    FogShape fogShape,
    double vignetteAlpha,
    int tintColorArgb,
    double particleDensity,
    double postFxSharpen
) {
    public RealmVisionCommand {
        fogStart = Math.max(0.0, fogStart);
        fogEnd = Math.max(fogStart, fogEnd);
        fogColorRgb = fogColorRgb & 0x00FFFFFF;
        fogShape = fogShape == null ? FogShape.CYLINDER : fogShape;
        vignetteAlpha = clamp01(vignetteAlpha);
        particleDensity = clamp01(particleDensity);
        postFxSharpen = clamp01(postFxSharpen);
    }

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }
}
