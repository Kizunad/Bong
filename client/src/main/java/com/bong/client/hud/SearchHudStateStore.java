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

    static synchronized SearchHudState snapshotAtNanos(long nowNanos) {
        if (terminalFlashExpired(snapshot.phase(), terminalPhaseStartedAtNanos, nowNanos)) {
            clear();
        }
        return snapshot;
    }

    public static synchronized void markStarted(String containerKindZh, int requiredTicks) {
        terminalPhaseStartedAtNanos = 0L;
        snapshot = SearchHudState.searching(safeKind(containerKindZh), Math.max(1, requiredTicks), 0);
    }

    public static synchronized void markProgress(String containerKindZh, int elapsedTicks, int requiredTicks) {
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

    static synchronized void markCompletedAtNanos(String containerKindZh, long nowNanos) {
        snapshot = SearchHudState.completed(safeKind(containerKindZh));
        terminalPhaseStartedAtNanos = nowNanos;
    }

    public static void markAborted(String containerKindZh, String reason) {
        markAbortedAtNanos(containerKindZh, reason, System.nanoTime());
    }

    static synchronized void markAbortedAtNanos(String containerKindZh, String reason, long nowNanos) {
        snapshot = SearchHudState.aborted(safeKind(containerKindZh), abortReason(reason));
        terminalPhaseStartedAtNanos = nowNanos;
    }

    public static synchronized void clearOnDisconnect() {
        clear();
    }

    public static synchronized void resetForTests() {
        clear();
    }

    private static void clear() {
        snapshot = SearchHudState.idle();
        terminalPhaseStartedAtNanos = 0L;
    }

    private static boolean terminalFlashExpired(
        SearchHudState.Phase phase,
        long startedAtNanos,
        long nowNanos
    ) {
        long ttlNanos = switch (phase) {
            case COMPLETED_FLASH -> COMPLETED_FLASH_TTL_NANOS;
            case ABORTED_FLASH -> ABORTED_FLASH_TTL_NANOS;
            case IDLE, SEARCHING -> 0L;
        };
        if (ttlNanos == 0L) {
            return false;
        }

        // System.nanoTime() 只承诺差值语义。long 减法在回绕时仍能正确表示远小于
        // 2^63ns 的短间隔；人为/测试时钟回拨则产生负差值，不会被误判为过期。
        return nowNanos - startedAtNanos >= ttlNanos;
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
