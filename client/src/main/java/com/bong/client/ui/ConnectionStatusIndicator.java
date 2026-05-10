package com.bong.client.ui;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;

import java.util.List;

public final class ConnectionStatusIndicator {
    public static final int GREEN = 0xFF44AA44;
    public static final int YELLOW = 0xFFCCAA44;
    public static final int RED = 0xFFAA4444;
    public static final long UNKNOWN_LATENCY_MS = -1L;
    private static final int DOT_SIZE = 6;

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

    public static List<HudRenderCommand> buildCommands(Snapshot snapshot, int screenWidth, int screenHeight) {
        if (snapshot == null || snapshot.status() == Status.HIDDEN || screenWidth <= 0 || screenHeight <= 0) {
            return List.of();
        }
        int x = Math.max(2, screenWidth - 18);
        int y = Math.max(2, screenHeight - 28);
        return List.of(HudRenderCommand.rect(HudRenderLayer.CONNECTION_STATUS, x, y, DOT_SIZE, DOT_SIZE, withAlpha(snapshot.color(), 102)));
    }

    private static int withAlpha(int color, int alpha) {
        return (Math.max(0, Math.min(255, alpha)) << 24) | (color & 0x00FFFFFF);
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
