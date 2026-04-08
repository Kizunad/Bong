package com.bong.client;

public final class EventAlertState {
    private static final long DEFAULT_ALERT_DURATION_MS = 5_000L;

    private static ActiveAlert activeAlert;

    private EventAlertState() {
    }

    public static synchronized void show(String message) {
        show(message, DEFAULT_ALERT_DURATION_MS, System.currentTimeMillis());
    }

    static synchronized void show(String message, long durationMs, long nowMs) {
        if (message == null || message.isBlank() || durationMs <= 0) {
            return;
        }

        activeAlert = new ActiveAlert(message, nowMs + durationMs);
    }

    public static synchronized ActiveAlert peek() {
        return peek(System.currentTimeMillis());
    }

    static synchronized ActiveAlert peek(long nowMs) {
        if (activeAlert == null) {
            return null;
        }

        if (nowMs >= activeAlert.expiresAtMs()) {
            activeAlert = null;
            return null;
        }

        return activeAlert;
    }

    static synchronized void clear() {
        activeAlert = null;
    }

    public record ActiveAlert(String message, long expiresAtMs) {
    }
}
