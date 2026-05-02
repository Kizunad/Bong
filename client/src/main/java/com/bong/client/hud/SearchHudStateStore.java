package com.bong.client.hud;

public final class SearchHudStateStore {
    private static volatile SearchHudState snapshot = SearchHudState.idle();

    private SearchHudStateStore() {
    }

    public static SearchHudState snapshot() {
        return snapshot;
    }

    public static void markStarted(String containerKindZh, int requiredTicks) {
        snapshot = SearchHudState.searching(safeKind(containerKindZh), Math.max(1, requiredTicks), 0);
    }

    public static void markProgress(String containerKindZh, int elapsedTicks, int requiredTicks) {
        snapshot = SearchHudState.searching(
            safeKind(containerKindZh),
            Math.max(1, requiredTicks),
            Math.max(0, elapsedTicks)
        );
    }

    public static void markCompleted(String containerKindZh) {
        snapshot = SearchHudState.completed(safeKind(containerKindZh));
    }

    public static void markAborted(String containerKindZh, String reason) {
        snapshot = SearchHudState.aborted(safeKind(containerKindZh), abortReason(reason));
    }

    public static void resetForTests() {
        snapshot = SearchHudState.idle();
    }

    private static String safeKind(String containerKindZh) {
        return containerKindZh == null || containerKindZh.isBlank() ? "容器" : containerKindZh;
    }

    private static SearchHudState.AbortReason abortReason(String reason) {
        return switch (reason == null ? "" : reason) {
            case "moved" -> SearchHudState.AbortReason.MOVED;
            case "combat" -> SearchHudState.AbortReason.COMBAT;
            case "damaged" -> SearchHudState.AbortReason.DAMAGED;
            case "cancelled" -> SearchHudState.AbortReason.CANCELLED;
            default -> SearchHudState.AbortReason.NONE;
        };
    }
}
