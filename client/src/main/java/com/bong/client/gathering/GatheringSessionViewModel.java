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
    private static final long RATIO_TOTAL_TICKS = 10_000L;
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
        } else if (totalTicks == 0L) {
            progressTicks = 0L;
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

    public static GatheringSessionViewModel createFromProgressRatio(
        String sessionId,
        double progressRatio,
        String targetName,
        String targetType,
        boolean interrupted,
        boolean completed,
        long updatedAtMillis
    ) {
        double normalizedProgress = Double.isFinite(progressRatio)
            ? Math.max(0.0, Math.min(1.0, progressRatio))
            : 0.0;
        long progressTicks = Math.round(normalizedProgress * RATIO_TOTAL_TICKS);
        return create(
            sessionId,
            progressTicks,
            RATIO_TOTAL_TICKS,
            targetName,
            targetType,
            "",
            "",
            interrupted,
            completed || normalizedProgress >= 1.0,
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

    public boolean hasPerfectQualityHint() {
        return "perfect".equals(qualityHint) || "perfect_possible".equals(qualityHint);
    }

    private static String normalize(String value) {
        return value == null ? "" : value.trim();
    }
}
