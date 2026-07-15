package com.bong.client.hud;

public final class SearchHudStateStore {
    static final long COMPLETED_FLASH_TTL_NANOS = 3_000_000_000L;
    static final long ABORTED_FLASH_TTL_NANOS = 1_000_000_000L;

    private static volatile SearchHudState snapshot = SearchHudState.idle();
    private static long terminalPhaseStartedAtNanos;

    private SearchHudStateStore() {
    }

    public static SearchHudState snapshot() {
        return snapshotAtNanos(System.nanoTime());
    }

    static SearchHudState snapshotAtNanos(long nowNanos) {
        return snapshot;
    }

    public static void markStarted(String containerKindZh, int requiredTicks) {
        terminalPhaseStartedAtNanos = 0L;
        snapshot = SearchHudState.searching(safeKind(containerKindZh), Math.max(1, requiredTicks), 0);
    }

    public static void markProgress(String containerKindZh, int elapsedTicks, int requiredTicks) {
        terminalPhaseStartedAtNanos = 0L;
        snapshot = SearchHudState.searching(
            safeKind(containerKindZh),
            Math.max(1, requiredTicks),
            Math.max(0, elapsedTicks)
        );
    }

    public static void markCompleted(String containerKindZh) {
        markCompletedAtNanos(containerKindZh, System.nanoTime());
    }

    static void markCompletedAtNanos(String containerKindZh, long nowNanos) {
        snapshot = SearchHudState.completed(safeKind(containerKindZh));
        terminalPhaseStartedAtNanos = nowNanos;
    }

    public static void markAborted(String containerKindZh, String reason) {
        markAbortedAtNanos(containerKindZh, reason, System.nanoTime());
    }

    static void markAbortedAtNanos(String containerKindZh, String reason, long nowNanos) {
        snapshot = SearchHudState.aborted(safeKind(containerKindZh), abortReason(reason));
        terminalPhaseStartedAtNanos = nowNanos;
    }

    public static void clearOnDisconnect() {
        clear();
    }

    public static void resetForTests() {
        clear();
    }

    private static void clear() {
        snapshot = SearchHudState.idle();
        terminalPhaseStartedAtNanos = 0L;
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
