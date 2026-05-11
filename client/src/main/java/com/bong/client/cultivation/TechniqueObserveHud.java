package com.bong.client.cultivation;

import com.bong.client.hud.BongToast;

public final class TechniqueObserveHud {
    public static final int INSIGHT_COLOR = 0xFFC4A94D;
    public static final int PROFICIENCY_COLOR = 0xFF9B7ED8;
    private static final long TOAST_DURATION_MS = 4_000L;

    private TechniqueObserveHud() {
    }

    public static void showObservedLearned(String techniqueDisplayName, long nowMillis) {
        BongToast.show("观摩领悟·" + displayNameOrFallback(techniqueDisplayName), INSIGHT_COLOR, nowMillis, TOAST_DURATION_MS);
    }

    public static void showProficiencyUp(String techniqueDisplayName, float proficiency, long nowMillis) {
        int percent = Math.max(0, Math.min(100, Math.round(proficiency * 100.0f)));
        BongToast.show(
            displayNameOrFallback(techniqueDisplayName) + " 熟练度 " + percent + "%",
            PROFICIENCY_COLOR,
            nowMillis,
            TOAST_DURATION_MS
        );
    }

    private static String displayNameOrFallback(String value) {
        String trimmed = value == null ? "" : value.trim();
        return trimmed.isEmpty() ? "涡流之法" : trimmed;
    }
}
