package com.bong.client.ui;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.text.Text;

import java.util.ArrayList;
import java.util.List;

public final class LoadingOverlay {
    public static final long RETRY_TIMEOUT_MS = 3_000L;
    public static final long LOST_TIMEOUT_MS = 10_000L;
    public static final long FADE_OUT_MS = 200L;
    private static final int INK_COLOR = 0xFF2A2A2A;
    private static final int TEXT_COLOR = 0xFFC0B090;

    private LoadingOverlay() {
    }

    public static Snapshot snapshot(long startedAtMs, long nowMs, boolean dataReady, boolean lowSpec) {
        long started = Math.max(0L, startedAtMs);
        long now = Math.max(started, nowMs);
        long elapsed = now - started;
        Phase phase;
        String message;
        if (dataReady) {
            phase = Phase.READY;
            message = "";
        } else if (elapsed >= LOST_TIMEOUT_MS) {
            phase = Phase.LOST;
            message = "天道失联";
        } else if (elapsed >= RETRY_TIMEOUT_MS) {
            phase = Phase.RETRY;
            message = "灵脉堵塞，稍后再试";
        } else {
            phase = Phase.LOADING;
            message = "凝神中...";
        }
        return new Snapshot(phase, message, elapsed, lowSpec ? List.of() : particles(elapsed), buttonLabels(phase));
    }

    public static PreloadFrame preloadFrame(
        long preloadStartedAtMs,
        long transitionStartedAtMs,
        int transitionDurationMs,
        long nowMs,
        boolean dataReady
    ) {
        long transitionEnd = Math.max(0L, transitionStartedAtMs) + Math.max(0, transitionDurationMs);
        boolean transitionRunning = nowMs < transitionEnd;
        boolean loadingVisible = !dataReady && !transitionRunning;
        boolean readyToOpen = dataReady && !transitionRunning;
        return new PreloadFrame(preloadStartedAtMs, transitionRunning, loadingVisible, readyToOpen);
    }

    public static void render(DrawContext context, MinecraftClient client, Snapshot snapshot) {
        if (context == null || client == null || client.getWindow() == null || snapshot == null || snapshot.phase() == Phase.READY) {
            return;
        }
        int width = client.getWindow().getScaledWidth();
        int height = client.getWindow().getScaledHeight();
        context.fill(0, 0, width, height, 0xB0000000);
        int cx = width / 2;
        int cy = height / 2;
        for (Particle p : snapshot.particles()) {
            int x = cx + p.x();
            int y = cy + p.y();
            int r = p.radius();
            context.fill(x - r, y - r, x + r, y + r, ScreenTransitionOverlay.withAlpha(INK_COLOR, p.alpha()));
        }
        context.drawCenteredTextWithShadow(client.textRenderer, Text.literal(snapshot.message()), cx, cy + 28, TEXT_COLOR);
        int buttonY = cy + 46;
        for (String label : snapshot.buttonLabels()) {
            int textWidth = client.textRenderer.getWidth(label);
            context.fill(cx - textWidth / 2 - 8, buttonY - 4, cx + textWidth / 2 + 8, buttonY + 12, 0x66202020);
            context.drawCenteredTextWithShadow(client.textRenderer, Text.literal(label), cx, buttonY, TEXT_COLOR);
            buttonY += 18;
        }
    }

    private static List<Particle> particles(long elapsedMs) {
        ArrayList<Particle> out = new ArrayList<>();
        double loop = (elapsedMs % 3000L) / 3000.0;
        for (int i = 0; i < 5; i++) {
            double angle = Math.PI * 2.0 * (i / 5.0) + loop * Math.PI * 0.7;
            int distance = (int) Math.round(6.0 + loop * 20.0 + i * 1.5);
            int x = (int) Math.round(Math.cos(angle) * distance);
            int y = (int) Math.round(Math.sin(angle) * distance);
            int alpha = (int) Math.round(76.0 * (1.0 - loop * 0.45));
            out.add(new Particle(x, y, 2 + (i % 2), alpha));
        }
        return List.copyOf(out);
    }

    private static List<String> buttonLabels(Phase phase) {
        return switch (phase) {
            case RETRY -> List.of("重试");
            case LOST -> List.of("重试", "返回主世界");
            default -> List.of();
        };
    }

    public enum Phase {
        LOADING,
        RETRY,
        LOST,
        READY
    }

    public record Snapshot(Phase phase, String message, long elapsedMs, List<Particle> particles, List<String> buttonLabels) {
        public Snapshot {
            message = message == null ? "" : message;
            particles = particles == null ? List.of() : List.copyOf(particles);
            buttonLabels = buttonLabels == null ? List.of() : List.copyOf(buttonLabels);
        }
    }

    public record Particle(int x, int y, int radius, int alpha) {
    }

    public record PreloadFrame(
        long preloadStartedAtMs,
        boolean transitionRunning,
        boolean loadingVisible,
        boolean readyToOpen
    ) {
    }
}
