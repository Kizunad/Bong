package com.bong.client.botany;

import java.util.List;

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
    List<String> hazardHints,
    double[] targetPos,
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
        List.of(),
        null,
        0L
    );

    public HarvestSessionViewModel {
        sessionId = normalize(sessionId);
        targetId = normalize(targetId);
        targetName = normalize(targetName);
        plantKindId = normalize(plantKindId);
        progress = clamp(progress);
        detail = normalize(detail);
        hazardHints = normalizeHazardHints(hazardHints);
        targetPos = normalizePos(targetPos);
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
        return create(
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
            List.of(),
            null,
            updatedAtMillis
        );
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
        double[] targetPos,
        long updatedAtMillis
    ) {
        return create(
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
            List.of(),
            targetPos,
            updatedAtMillis
        );
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
        List<String> hazardHints,
        double[] targetPos,
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
            hazardHints,
            targetPos,
            updatedAtMillis
        );
    }

    public boolean isEmpty() {
        return sessionId.isEmpty();
    }

    public boolean interactive() {
        return !isEmpty() && !interrupted && !completed;
    }

    public boolean hasTargetPos() {
        return targetPos != null && targetPos.length == 3;
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
            hazardHints,
            targetPos,
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
            hazardHints,
            targetPos,
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

    private static double[] normalizePos(double[] src) {
        if (src == null || src.length != 3) {
            return null;
        }
        for (double v : src) {
            if (!Double.isFinite(v)) {
                return null;
            }
        }
        return new double[] { src[0], src[1], src[2] };
    }

    private static List<String> normalizeHazardHints(List<String> src) {
        if (src == null || src.isEmpty()) {
            return List.of();
        }
        return src.stream()
            .map(HarvestSessionViewModel::normalize)
            .filter(s -> !s.isEmpty())
            .limit(4)
            .toList();
    }
}
