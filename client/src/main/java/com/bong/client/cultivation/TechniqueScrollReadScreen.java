package com.bong.client.cultivation;

import com.bong.client.hud.BongToast;

public final class TechniqueScrollReadScreen {
    public static final int LEARNED_COLOR = 0xFFC4A94D;
    public static final int REQUESTED_COLOR = 0xFF9B7ED8;
    private static final long TOAST_DURATION_MS = 3_000L;

    private TechniqueScrollReadScreen() {
    }

    public static boolean isWoliuTechniqueId(String techniqueId) {
        if (techniqueId == null) return false;
        return switch (techniqueId.trim()) {
            case "woliu.vortex",
                 "woliu.hold",
                 "woliu.burst",
                 "woliu.mouth",
                 "woliu.pull",
                 "woliu.vacuum_palm",
                 "woliu.vortex_shield",
                 "woliu.heart",
                 "woliu.vacuum_lock",
                 "woliu.vortex_resonance",
                 "woliu.turbulence_burst" -> true;
            default -> false;
        };
    }

    public static String learnedText(String techniqueDisplayName) {
        return "习得·" + displayNameOrFallback(techniqueDisplayName);
    }

    public static void showLearned(String techniqueDisplayName, long nowMillis) {
        BongToast.show(learnedText(techniqueDisplayName), LEARNED_COLOR, nowMillis, TOAST_DURATION_MS);
    }

    public static void showReadRequested(String itemDisplayName, long nowMillis) {
        BongToast.show("研读·" + displayNameOrFallback(itemDisplayName), REQUESTED_COLOR, nowMillis, TOAST_DURATION_MS);
    }

    private static String displayNameOrFallback(String value) {
        String trimmed = value == null ? "" : value.trim();
        return trimmed.isEmpty() ? "涡流之法" : trimmed;
    }
}
