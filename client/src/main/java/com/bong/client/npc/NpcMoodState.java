package com.bong.client.npc;

import java.util.Locale;

public record NpcMoodState(
    int entityId,
    String mood,
    double threatLevel,
    String qiLevelHint,
    String innerMonologue,
    long updatedAtMillis
) {
    public NpcMoodState {
        mood = normalizeMood(mood);
        threatLevel = clamp01(threatLevel);
        qiLevelHint = blankToNull(qiLevelHint);
        innerMonologue = blankToNull(innerMonologue);
        updatedAtMillis = Math.max(0L, updatedAtMillis);
    }

    public boolean hostile() {
        return "hostile".equals(mood);
    }

    public boolean fearful() {
        return "fearful".equals(mood);
    }

    public boolean alert() {
        return "alert".equals(mood);
    }

    private static String normalizeMood(String value) {
        if (value == null || value.isBlank()) {
            return "neutral";
        }
        String normalized = value.trim().toLowerCase(Locale.ROOT);
        return switch (normalized) {
            case "neutral", "alert", "hostile", "fearful" -> normalized;
            default -> "neutral";
        };
    }

    private static String blankToNull(String value) {
        return value == null || value.isBlank() ? null : value.trim();
    }

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }
}
