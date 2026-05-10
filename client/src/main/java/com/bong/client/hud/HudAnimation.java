package com.bong.client.hud;

public final class HudAnimation {
    public static final long TOAST_SLIDE_IN_MS = 200L;
    public static final long TOAST_SLIDE_OUT_MS = 300L;
    public static final long TYPEWRITER_CHAR_MS = 30L;

    private HudAnimation() {
    }

    public static double smoothLerp(double previous, double target, double alpha) {
        double safePrevious = finiteOrZero(previous);
        double safeTarget = finiteOrZero(target);
        double safeAlpha = clamp01(alpha);
        return safePrevious + (safeTarget - safePrevious) * safeAlpha;
    }

    public static int smoothFillWidth(double previousRatio, double targetRatio, int width, double alpha) {
        int safeWidth = Math.max(0, width);
        double smoothed = smoothLerp(clamp01(previousRatio), clamp01(targetRatio), alpha);
        return Math.max(0, Math.min(safeWidth, (int) Math.round(smoothed * safeWidth)));
    }

    public static int toastSlideOffset(long shownAtMs, long expiresAtMs, long nowMs, int travelPixels) {
        int safeTravel = Math.max(0, travelPixels);
        long safeNow = Math.max(0L, nowMs);
        long safeShownAt = Math.max(0L, shownAtMs);
        long safeExpiresAt = Math.max(safeShownAt, expiresAtMs);
        long age = Math.max(0L, safeNow - safeShownAt);
        long remaining = Math.max(0L, safeExpiresAt - safeNow);

        if (age < TOAST_SLIDE_IN_MS) {
            double t = age / (double) TOAST_SLIDE_IN_MS;
            return (int) Math.round(safeTravel * (1.0 - easeOutCubic(t)));
        }
        if (remaining < TOAST_SLIDE_OUT_MS) {
            double t = 1.0 - remaining / (double) TOAST_SLIDE_OUT_MS;
            return (int) Math.round(safeTravel * easeInCubic(t));
        }
        return 0;
    }

    public static String typewriterText(String text, long createdAtMs, long nowMs) {
        String safeText = text == null ? "" : text;
        if (safeText.isEmpty()) {
            return "";
        }
        long elapsed = Math.max(0L, Math.max(0L, nowMs) - Math.max(0L, createdAtMs));
        int visible = (int) Math.min(safeText.length(), elapsed / TYPEWRITER_CHAR_MS + 1);
        return safeText.substring(0, visible);
    }

    public static int alphaForFlash(long startedAtMs, long durationMs, long nowMs, int maxAlpha) {
        long safeDuration = Math.max(1L, durationMs);
        long elapsed = Math.max(0L, Math.max(0L, nowMs) - Math.max(0L, startedAtMs));
        if (elapsed >= safeDuration) {
            return 0;
        }
        double remaining = 1.0 - elapsed / (double) safeDuration;
        return HudTextHelper.clampAlpha((int) Math.round(Math.max(0, maxAlpha) * remaining));
    }

    private static double easeOutCubic(double t) {
        double clamped = clamp01(t);
        double inv = 1.0 - clamped;
        return 1.0 - inv * inv * inv;
    }

    private static double easeInCubic(double t) {
        double clamped = clamp01(t);
        return clamped * clamped * clamped;
    }

    private static double finiteOrZero(double value) {
        return Double.isFinite(value) ? value : 0.0;
    }

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }
}
