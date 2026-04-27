package com.bong.client.tsy;

import java.util.List;

public record ExtractState(
    List<RiftPortalView> portals,
    Long activePortalEntityId,
    String activePortalKind,
    int elapsedTicks,
    int requiredTicks,
    boolean extracting,
    String message,
    int messageColor,
    long messageUntilMs,
    String collapsingFamilyId,
    long collapseStartedAtMs,
    int collapseRemainingTicksAtStart,
    long screenFlashUntilMs,
    int screenFlashColor,
    long updatedAtMs
) {
    public static ExtractState empty() {
        return new ExtractState(
            List.of(),
            null,
            "",
            0,
            0,
            false,
            "",
            0xFFFFFFFF,
            0L,
            "",
            0L,
            0,
            0L,
            0,
            0L
        );
    }

    public boolean hasActivePortal() {
        return activePortalEntityId != null;
    }

    public boolean hasTimedMessage(long nowMs) {
        return message != null && !message.isBlank() && nowMs <= messageUntilMs;
    }

    public boolean collapseActive(long nowMs) {
        return collapsingFamilyId != null
            && !collapsingFamilyId.isBlank()
            && collapseRemainingTicks(nowMs) > 0;
    }

    public int collapseRemainingTicks(long nowMs) {
        if (collapseStartedAtMs <= 0 || collapseRemainingTicksAtStart <= 0) {
            return 0;
        }
        long elapsedTicks = Math.max(0L, nowMs - collapseStartedAtMs) / 50L;
        return (int) Math.max(0L, collapseRemainingTicksAtStart - elapsedTicks);
    }

    public boolean screenFlashActive(long nowMs) {
        return screenFlashUntilMs > 0 && nowMs <= screenFlashUntilMs;
    }
}
