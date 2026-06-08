package com.bong.client.tiandao;

public record TiandaoPresenceState(
    boolean active,
    String response,
    double level,
    String zone,
    double zoneSpiritQi,
    int vignetteRgb,
    double vignetteAlpha,
    double shakeIntensity,
    double saturation,
    long tick
) {
    public static TiandaoPresenceState empty() {
        return new TiandaoPresenceState(false, "none", 0.0, "", 0.0, 0, 0.0, 0.0, 1.0, 0L);
    }

    public TiandaoPresenceState {
        response = normalizeResponse(response);
        zone = zone == null ? "" : zone;
        level = clamp(level, 0.0, 100.0);
        zoneSpiritQi = finiteOr(zoneSpiritQi, 0.0);
        vignetteRgb = vignetteRgb & 0x00FFFFFF;
        vignetteAlpha = clamp(vignetteAlpha, 0.0, 1.0);
        shakeIntensity = clamp(shakeIntensity, 0.0, 1.0);
        saturation = clamp(saturation, 0.0, 1.0);
        active = active && !"none".equals(response) && vignetteAlpha > 0.0;
    }

    public int vignetteArgb(long nowMillis) {
        double alpha = vignetteAlpha;
        if ("annihilate".equals(response)) {
            double phase = (Math.sin((nowMillis / 1000.0) * Math.PI) + 1.0) * 0.5;
            alpha = vignetteAlpha * (0.7 + 0.3 * phase);
        }
        int a = (int) Math.round(clamp(alpha, 0.0, 1.0) * 255.0);
        return (a << 24) | vignetteRgb;
    }

    public int tintArgb() {
        if (saturation >= 0.99) {
            return 0;
        }
        int alpha = (int) Math.round((1.0 - saturation) * 96.0);
        return (alpha << 24);
    }

    private static String normalizeResponse(String value) {
        if (value == null) {
            return "none";
        }
        return switch (value.trim().toLowerCase()) {
            case "watch", "pressure", "tribulation", "annihilate" -> value.trim().toLowerCase();
            default -> "none";
        };
    }

    private static double finiteOr(double value, double fallback) {
        return Double.isFinite(value) ? value : fallback;
    }

    private static double clamp(double value, double min, double max) {
        if (!Double.isFinite(value)) {
            return min;
        }
        return Math.max(min, Math.min(max, value));
    }
}
