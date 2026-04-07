package com.bong.client;

public final class NarrationToastState {
    private static ActiveToast currentToast;

    private NarrationToastState() {
    }

    public static synchronized void show(String text, int color, long durationMs) {
        show(text, color, durationMs, System.currentTimeMillis());
    }

    static synchronized void show(String text, int color, long durationMs, long nowMs) {
        if (text == null || text.isBlank() || durationMs <= 0) {
            return;
        }

        currentToast = new ActiveToast(text, color, nowMs + durationMs);
    }

    public static synchronized ActiveToast peek() {
        return peek(System.currentTimeMillis());
    }

    static synchronized ActiveToast peek(long nowMs) {
        if (currentToast == null) {
            return null;
        }

        if (nowMs >= currentToast.expiresAtMs()) {
            currentToast = null;
            return null;
        }

        return currentToast;
    }

    static synchronized void clear() {
        currentToast = null;
    }

    public record ActiveToast(String text, int color, long expiresAtMs) {
    }
}
