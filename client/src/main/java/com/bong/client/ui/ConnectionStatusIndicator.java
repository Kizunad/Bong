package com.bong.client.ui;

import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;

import java.util.List;

public final class ConnectionStatusIndicator {
    public static final int GREEN = 0xFF44AA44;
    public static final int YELLOW = 0xFFCCAA44;
    public static final int RED = 0xFFAA4444;
    private static final int DOT_SIZE = 6;

    private ConnectionStatusIndicator() {
    }

    public static Snapshot evaluate(boolean connected, long latencyMs, long disconnectedDurationMs, long lastResponseAgeMs) {
        if (connected && lastResponseAgeMs <= 5_000L) {
            return new Snapshot(Status.GREEN, GREEN, Math.max(0L, latencyMs), 0L, "天道连接 · 延迟 " + Math.max(0L, latencyMs) + "ms");
        }
        if (connected || disconnectedDurationMs < 10_000L) {
            long duration = connected ? Math.max(0L, lastResponseAgeMs) : Math.max(0L, disconnectedDurationMs);
            return new Snapshot(Status.YELLOW, YELLOW, Math.max(0L, latencyMs), duration, "天道迟滞 · " + duration / 1000L + "s");
        }
        long duration = Math.max(0L, disconnectedDurationMs);
        return new Snapshot(Status.RED, RED, Math.max(0L, latencyMs), duration, "天道失联 · 断开 " + duration / 1000L + "s");
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
