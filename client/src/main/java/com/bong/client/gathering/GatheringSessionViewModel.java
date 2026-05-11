package com.bong.client.gathering;

import java.util.Locale;

public record GatheringSessionViewModel(
    String sessionId,
    long progressTicks,
    long totalTicks,
    String targetName,
    String targetType,
    String qualityHint,
    String toolUsed,
    boolean interrupted,
    boolean completed,
    long updatedAtMillis
) {
    private static final GatheringSessionViewModel EMPTY = new GatheringSessionViewModel(
        "",
        0L,
        0L,
        "",
        "",
        "",
        "",
        false,
        false,
        0L
    );

    public GatheringSessionViewModel {
        sessionId = normalize(sessionId);
        progressTicks = Math.max(0L, progressTicks);
        totalTicks = Math.max(0L, totalTicks);
        if (progressTicks > totalTicks && totalTicks > 0L) {
            progressTicks = totalTicks;
        }
        targetName = normalize(targetName);
        targetType = normalize(targetType).toLowerCase(Locale.ROOT);
        qualityHint = normalize(qualityHint).toLowerCase(Locale.ROOT);
        toolUsed = normalize(toolUsed);
        updatedAtMillis = Math.max(0L, updatedAtMillis);
    }

    public static GatheringSessionViewModel empty() {
        return EMPTY;
    }

    public static GatheringSessionViewModel create(
        String sessionId,
        long progressTicks,
        long totalTicks,
        String targetName,
        String targetType,
        String qualityHint,
        String toolUsed,
        boolean interrupted,
        boolean completed,
        long updatedAtMillis
    ) {
        if (normalize(sessionId).isEmpty()) {
            return empty();
        }
        return new GatheringSessionViewModel(
            sessionId,
            progressTicks,
            totalTicks,
            targetName,
            targetType,
            qualityHint,
            toolUsed,
            interrupted,
            completed,
            updatedAtMillis
        );
    }

    public boolean isEmpty() {
        return sessionId.isEmpty();
    }

    public boolean active() {
        return !isEmpty() && !interrupted && !completed;
    }

    public double progressRatio() {
        if (totalTicks <= 0L) {
            return completed ? 1.0 : 0.0;
        }
        return Math.max(0.0, Math.min(1.0, (double) progressTicks / (double) totalTicks));
    }

    public String displayTargetName() {
        if (!targetName.isEmpty()) {
            return targetName;
        }
        return switch (targetType) {
            case "ore" -> "矿脉";
            case "wood" -> "木材";
            default -> "草药";
        };
    }

    public String qualityLabel() {
        return switch (qualityHint) {
            case "perfect", "perfect_possible" -> "极品";
            case "fine", "fine_likely" -> "优良";
            case "normal" -> "";
            default -> "";
        };
    }

    private static String normalize(String value) {
        return value == null ? "" : value.trim();
    }
}
