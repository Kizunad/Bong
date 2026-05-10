package com.bong.client.ui;

import net.minecraft.client.gui.screen.Screen;

import java.util.Objects;

public final class ScreenTransition {
    public static final int MIN_DURATION_MS = 100;
    public static final int MAX_DURATION_MS = 600;

    private ScreenTransition() {
    }

    public static TransitionHandle play(
        Screen oldScreen,
        Screen newScreen,
        Type type,
        int durationMs,
        Easing easing,
        Runnable callback
    ) {
        Type normalizedType = type == null ? Type.NONE : type;
        TransitionHandle handle = new TransitionHandle(
            oldScreen,
            newScreen,
            normalizedType,
            clampDuration(normalizedType, durationMs),
            easing == null ? Easing.EASE_OUT_CUBIC : easing,
            nowMillis(),
            callback
        );
        if (handle.durationMs() == 0) {
            handle.complete();
        }
        return handle;
    }

    public static Frame sample(Type type, int durationMs, Easing easing, long startedAtMs, long nowMs, int width, int height) {
        Type safeType = type == null ? Type.NONE : type;
        int safeDuration = clampDuration(safeType, durationMs);
        long safeNow = Math.max(0L, nowMs);
        long safeStarted = Math.max(0L, startedAtMs);
        int safeWidth = Math.max(0, width);
        int safeHeight = Math.max(0, height);
        if (safeType == Type.NONE || safeDuration == 0) {
            return new Frame(1.0, 1.0, 0.0, 0, 0, 1.0, false, true);
        }

        double linear = Math.max(0.0, Math.min(1.0, (safeNow - safeStarted) / (double) safeDuration));
        double eased = applyEasing(easing, linear);
        int offsetX = 0;
        int offsetY = 0;
        double scale = 1.0;

        switch (safeType) {
            case SLIDE_UP -> offsetY = (int) Math.round(safeHeight * (1.0 - eased));
            case SLIDE_DOWN -> offsetY = (int) Math.round(safeHeight * eased);
            case SLIDE_RIGHT -> offsetX = (int) Math.round(safeWidth * (1.0 - eased));
            case SLIDE_LEFT -> offsetX = -(int) Math.round(safeWidth * eased);
            case SCALE_UP -> scale = 0.8 + 0.2 * eased;
            case SCALE_DOWN -> scale = 1.0 - 0.2 * eased;
            default -> {
            }
        }

        return new Frame(eased, eased, 1.0 - eased, offsetX, offsetY, scale, linear < 1.0, linear >= 1.0);
    }

    public static double applyEasing(Easing easing, double t) {
        double clamped = clamp01(t);
        return switch (easing == null ? Easing.EASE_OUT_CUBIC : easing) {
            case EASE_OUT_CUBIC -> {
                double inv = 1.0 - clamped;
                yield 1.0 - inv * inv * inv;
            }
            case EASE_OUT_QUAD -> {
                double inv = 1.0 - clamped;
                yield 1.0 - inv * inv;
            }
            case LINEAR -> clamped;
        };
    }

    public static int clampDuration(Type type, int durationMs) {
        if (type == Type.NONE || durationMs <= 0) {
            return 0;
        }
        return Math.max(MIN_DURATION_MS, Math.min(MAX_DURATION_MS, durationMs));
    }

    public static long nowMillis() {
        return System.nanoTime() / 1_000_000L;
    }

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }

    public enum Type {
        SLIDE_UP,
        SLIDE_DOWN,
        SLIDE_RIGHT,
        SLIDE_LEFT,
        FADE,
        SCALE_UP,
        SCALE_DOWN,
        NONE
    }

    public enum Easing {
        EASE_OUT_CUBIC,
        EASE_OUT_QUAD,
        LINEAR
    }

    public record Frame(
        double progress,
        double newAlpha,
        double oldAlpha,
        int offsetX,
        int offsetY,
        double scale,
        boolean inputLocked,
        boolean finished
    ) {
    }

    public static final class TransitionHandle {
        private final Screen oldScreen;
        private final Screen newScreen;
        private final Type type;
        private final int durationMs;
        private final Easing easing;
        private final long startedAtMs;
        private final Runnable callback;
        private boolean cancelled;
        private boolean completed;

        private TransitionHandle(
            Screen oldScreen,
            Screen newScreen,
            Type type,
            int durationMs,
            Easing easing,
            long startedAtMs,
            Runnable callback
        ) {
            this.oldScreen = oldScreen;
            this.newScreen = newScreen;
            this.type = Objects.requireNonNull(type, "type");
            this.durationMs = Math.max(0, durationMs);
            this.easing = Objects.requireNonNull(easing, "easing");
            this.startedAtMs = Math.max(0L, startedAtMs);
            this.callback = callback;
        }

        public Frame sample(long nowMs, int width, int height) {
            return ScreenTransition.sample(type, durationMs, easing, startedAtMs, nowMs, width, height);
        }

        public void cancel() {
            this.cancelled = true;
        }

        public void complete() {
            if (cancelled || completed) {
                return;
            }
            completed = true;
            if (callback != null) {
                callback.run();
            }
        }

        public Screen oldScreen() {
            return oldScreen;
        }

        public Screen newScreen() {
            return newScreen;
        }

        public Type type() {
            return type;
        }

        public int durationMs() {
            return durationMs;
        }

        public Easing easing() {
            return easing;
        }

        public long startedAtMs() {
            return startedAtMs;
        }

        public boolean cancelled() {
            return cancelled;
        }

        public boolean completed() {
            return completed;
        }
    }
}
