package com.bong.client.botany;

public record HarvestSessionViewModel(
    String sessionId,
    String targetId,
    String targetName,
    String plantKindId,
    BotanyHarvestMode mode,
    double progress,
    boolean autoSelectable,
    boolean requestPending,
    boolean interrupted,
    boolean completed,
    String detail,
    long updatedAtMillis
) {
    private static final HarvestSessionViewModel EMPTY = new HarvestSessionViewModel(
        "",
        "",
        "",
        "",
        null,
        0.0,
        true,
        false,
        false,
        false,
        "",
        0L
    );

    public HarvestSessionViewModel {
        sessionId = normalize(sessionId);
        targetId = normalize(targetId);
        targetName = normalize(targetName);
        plantKindId = normalize(plantKindId);
        progress = clamp(progress);
        detail = normalize(detail);
        updatedAtMillis = Math.max(0L, updatedAtMillis);
    }

    public static HarvestSessionViewModel empty() {
        return EMPTY;
    }

    public static HarvestSessionViewModel create(
        String sessionId,
        String targetId,
        String targetName,
        String plantKindId,
        BotanyHarvestMode mode,
        double progress,
        boolean autoSelectable,
        boolean requestPending,
        boolean interrupted,
        boolean completed,
        String detail,
        long updatedAtMillis
    ) {
        if (normalize(sessionId).isEmpty()) {
            return empty();
        }
        return new HarvestSessionViewModel(
            sessionId,
            targetId,
            targetName,
            plantKindId,
            mode,
            progress,
            autoSelectable,
            requestPending,
            interrupted,
            completed,
            detail,
            updatedAtMillis
        );
    }

    public boolean isEmpty() {
        return sessionId.isEmpty();
    }

    public boolean interactive() {
        return !isEmpty() && !interrupted && !completed;
    }

    public String displayTargetName() {
        if (!targetName.isEmpty()) {
            return targetName;
        }
        if (!plantKindId.isEmpty()) {
            return plantKindId;
        }
        return "未知灵草";
    }

    public HarvestSessionViewModel withRequestedMode(BotanyHarvestMode nextMode, long nowMillis) {
        if (isEmpty()) {
            return this;
        }
        return new HarvestSessionViewModel(
            sessionId,
            targetId,
            targetName,
            plantKindId,
            nextMode,
            progress,
            autoSelectable,
            true,
            false,
            false,
            detail,
            nowMillis
        );
    }

    public HarvestSessionViewModel locallyInterrupted(String reason, long nowMillis) {
        if (isEmpty()) {
            return this;
        }
        return new HarvestSessionViewModel(
            sessionId,
            targetId,
            targetName,
            plantKindId,
            mode,
            progress,
            autoSelectable,
            false,
            true,
            false,
            reason,
            nowMillis
        );
    }

    private static String normalize(String value) {
        return value == null ? "" : value.trim();
    }

    private static double clamp(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }
}
