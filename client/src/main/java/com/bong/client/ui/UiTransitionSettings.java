package com.bong.client.ui;

import com.bong.client.BongClientFeatures;

public final class UiTransitionSettings {
    private static volatile boolean enabled = BongClientFeatures.ENABLE_UI_TRANSITIONS;
    private static volatile boolean lowSpecFallback;
    private static volatile boolean lowFpsToastShown;

    private UiTransitionSettings() {
    }

    public static boolean enabled() {
        return enabled;
    }

    public static boolean lowSpecFallback() {
        return lowSpecFallback;
    }

    public static int durationFor(int configuredDurationMs) {
        if (!enabled) {
            return 0;
        }
        return Math.max(0, configuredDurationMs);
    }

    public static FpsDecision observeFrameRate(double fps) {
        if (!Double.isFinite(fps) || fps <= 0.0) {
            return new FpsDecision(false, false);
        }
        if (fps < 30.0) {
            boolean shouldToast = !lowFpsToastShown;
            lowFpsToastShown = true;
            return new FpsDecision(shouldToast, true);
        }
        return new FpsDecision(false, false);
    }

    public static void setEnabledForTests(boolean nextEnabled) {
        enabled = nextEnabled;
    }

    public static void setLowSpecFallbackForTests(boolean nextLowSpecFallback) {
        lowSpecFallback = nextLowSpecFallback;
    }

    public static void resetForTests() {
        enabled = BongClientFeatures.ENABLE_UI_TRANSITIONS;
        lowSpecFallback = false;
        lowFpsToastShown = false;
    }

    public record FpsDecision(boolean showToast, boolean fallbackSuggested) {
    }
}
