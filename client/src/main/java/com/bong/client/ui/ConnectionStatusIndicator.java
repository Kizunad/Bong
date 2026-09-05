package com.bong.client.ui;



public final class ConnectionStatusIndicator {
    public static final int GREEN = 0xFF44AA44;
    public static final int YELLOW = 0xFFCCAA44;
    public static final int RED = 0xFFAA4444;
    public static final long UNKNOWN_LATENCY_MS = -1L;

    private ConnectionStatusIndicator() {
    }

    public static Snapshot evaluate(boolean connected, long latencyMs, long disconnectedDurationMs, long lastResponseAgeMs) {
        if (connected && lastResponseAgeMs <= 5_000L) {
            long safeLatency = sanitizeLatency(latencyMs);
            return new Snapshot(Status.GREEN, GREEN, safeLatency, 0L, "天道连接 · 延迟 " + latencyLabel(safeLatency));
        }
        if (connected || disconnectedDurationMs < 10_000L) {
            long safeLatency = sanitizeLatency(latencyMs);
            long duration = connected ? Math.max(0L, lastResponseAgeMs) : Math.max(0L, disconnectedDurationMs);
            return new Snapshot(Status.YELLOW, YELLOW, safeLatency, duration, "天道迟滞 · " + duration / 1000L + "s · 延迟 " + latencyLabel(safeLatency));
        }
        long duration = Math.max(0L, disconnectedDurationMs);
        return new Snapshot(Status.RED, RED, UNKNOWN_LATENCY_MS, duration, "天道失联 · 断开 " + duration / 1000L + "s");
    }

    private static long sanitizeLatency(long latencyMs) {
        return latencyMs < 0L ? UNKNOWN_LATENCY_MS : latencyMs;
    }

    private static String latencyLabel(long latencyMs) {
        return latencyMs < 0L ? "--" : latencyMs + "ms";
    }

    public enum Status {
        HIDDEN,
        GREEN,
        YELLOW,
        RED
    }

    public record Snapshot(Status status, int color, long latencyMs, long disconnectedDurationMs, String tooltip) {
        public static Snapshot hidden() {
            return new Snapshot(Status.HIDDEN, 0, 0L, 0L, "");
        }

        public Snapshot {
            status = status == null ? Status.HIDDEN : status;
            tooltip = tooltip == null ? "" : tooltip;
        }
    }
}
