package com.bong.client.hud;

public final class HudTextHelper {
    private static final String ELLIPSIS = "...";

    private HudTextHelper() {
    }

    public static int clampAlpha(int alpha) {
        return Math.max(0, Math.min(255, alpha));
    }

    public static int withAlpha(int rgb, int alpha) {
        return (clampAlpha(alpha) << 24) | (rgb & 0x00FFFFFF);
    }

    public static String clipToWidth(String text, int maxWidth, WidthMeasurer widthMeasurer) {
        if (text == null || widthMeasurer == null || maxWidth <= 0) {
            return "";
        }

        String normalized = text.trim();
        if (normalized.isEmpty()) {
            return "";
        }

        if (safeWidth(normalized, widthMeasurer) <= maxWidth) {
            return normalized;
        }

        if (safeWidth(ELLIPSIS, widthMeasurer) > maxWidth) {
            return "";
        }

        for (int endIndex = normalized.length(); endIndex > 0; endIndex--) {
            String base = normalized.substring(0, endIndex).trim();
            String candidate = base.isEmpty() ? ELLIPSIS : base + ELLIPSIS;
            if (safeWidth(candidate, widthMeasurer) <= maxWidth) {
                return candidate;
            }
        }

        return ELLIPSIS;
    }

    private static int safeWidth(String text, WidthMeasurer widthMeasurer) {
        if (text == null || text.isEmpty()) {
            return 0;
        }
        return Math.max(0, widthMeasurer.measure(text));
    }

    @FunctionalInterface
    public interface WidthMeasurer {
        int measure(String text);
    }
}
