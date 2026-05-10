package com.bong.client.environment;

public record EnvironmentFogCommand(
    double fogStart,
    double fogEnd,
    int fogColorRgb,
    int skyColorRgb,
    double density
) {
    public EnvironmentFogCommand {
        fogStart = Math.max(0.0, fogStart);
        fogEnd = Math.max(fogStart, fogEnd);
        fogColorRgb = fogColorRgb & 0x00FFFFFF;
        skyColorRgb = skyColorRgb & 0x00FFFFFF;
        density = clamp01(density);
    }

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }
}
