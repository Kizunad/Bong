package com.bong.client;

import java.util.Objects;

public final class NarrationState {
    private static final long SYSTEM_WARNING_TOAST_DURATION_MS = 5_000L;
    private static final long ERA_DECREE_TOAST_DURATION_MS = 8_000L;
    private static final int SYSTEM_WARNING_TOAST_COLOR = 0xFF5555;
    private static final int ERA_DECREE_TOAST_COLOR = 0xFFD700;

    private static NarrationSnapshot latestNarration;

    private NarrationState() {
    }

    public static NarrationSnapshot recordNarration(BongServerPayload.Narration narration, ChatSink chatSink) {
        return recordNarration(narration, System.currentTimeMillis(), chatSink);
    }

    static NarrationSnapshot recordNarration(BongServerPayload.Narration narration, long nowMs, ChatSink chatSink) {
        Objects.requireNonNull(narration, "narration");
        Objects.requireNonNull(chatSink, "chatSink");

        NarrationSnapshot snapshot = snapshotOf(narration, nowMs);
        latestNarration = snapshot;
        chatSink.accept(snapshot);
        return snapshot;
    }

    static NarrationSnapshot snapshotOf(BongServerPayload.Narration narration, long nowMs) {
        Objects.requireNonNull(narration, "narration");

        long expiresAtMs = shouldShowToast(narration.style())
                ? nowMs + toastDurationMillis(narration.style())
                : 0L;

        return new NarrationSnapshot(
                narration.scope(),
                narration.text(),
                narration.style(),
                formatChatLine(narration),
                nowMs,
                expiresAtMs
        );
    }

    static String formatChatLine(BongServerPayload.Narration narration) {
        return switch (narration.style()) {
            case "system_warning" -> "[天道警示] " + narration.text();
            case "perception" -> "[感知] " + narration.text();
            case "narration" -> "[叙事] " + narration.text();
            case "era_decree" -> "[时代宣令] " + narration.text();
            default -> narration.text();
        };
    }

    static boolean shouldShowToast(String style) {
        return "system_warning".equals(style) || "era_decree".equals(style);
    }

    static long toastDurationMillis(String style) {
        return "era_decree".equals(style) ? ERA_DECREE_TOAST_DURATION_MS : SYSTEM_WARNING_TOAST_DURATION_MS;
    }

    static int toastColor(String style) {
        return "era_decree".equals(style) ? ERA_DECREE_TOAST_COLOR : SYSTEM_WARNING_TOAST_COLOR;
    }

    static NarrationSnapshot getLatestNarration() {
        return latestNarration;
    }

    public static ToastState getCurrentToast() {
        return getCurrentToast(System.currentTimeMillis());
    }

    static ToastState getCurrentToast(long nowMs) {
        NarrationSnapshot snapshot = latestNarration;
        if (snapshot == null || !shouldShowToast(snapshot.style()) || nowMs >= snapshot.expiresAtMs()) {
            return null;
        }

        return new ToastState(snapshot.chatLine(), toastColor(snapshot.style()), snapshot.expiresAtMs());
    }

    public static void clear() {
        latestNarration = null;
    }

    @FunctionalInterface
    interface ChatSink {
        void accept(NarrationSnapshot snapshot);
    }

    record NarrationSnapshot(String scope, String text, String style, String chatLine, long recordedAtMs, long expiresAtMs) {
        NarrationSnapshot {
            Objects.requireNonNull(scope, "scope");
            Objects.requireNonNull(text, "text");
            Objects.requireNonNull(style, "style");
            Objects.requireNonNull(chatLine, "chatLine");
        }
    }

    public record ToastState(String text, int color, long expiresAtMs) {
        public ToastState {
            Objects.requireNonNull(text, "text");
        }
    }
}
