package com.bong.client.state;

import java.util.Objects;

public final class VisualEffectState {
    private final EffectType effectType;
    private final double intensity;
    private final long durationMillis;
    private final long startedAtMillis;

    private VisualEffectState(EffectType effectType, double intensity, long durationMillis, long startedAtMillis) {
        this.effectType = Objects.requireNonNull(effectType, "effectType");
        this.intensity = intensity;
        this.durationMillis = durationMillis;
        this.startedAtMillis = startedAtMillis;
    }

    public static VisualEffectState none() {
        return new VisualEffectState(EffectType.NONE, 0.0, 0L, 0L);
    }

    public static VisualEffectState create(String effectType, double intensity, long durationMillis, long startedAtMillis) {
        EffectType normalizedType = EffectType.fromWireName(effectType);
        double normalizedIntensity = clamp(intensity, 0.0, 1.0);
        long normalizedDuration = Math.max(0L, durationMillis);
        long normalizedStart = Math.max(0L, startedAtMillis);

        if (normalizedType == EffectType.NONE || normalizedIntensity == 0.0 || normalizedDuration == 0L) {
            return none();
        }

        return new VisualEffectState(normalizedType, normalizedIntensity, normalizedDuration, normalizedStart);
    }

    private static double clamp(double value, double min, double max) {
        if (!Double.isFinite(value)) {
            return min;
        }
        return Math.max(min, Math.min(max, value));
    }

    public EffectType effectType() {
        return effectType;
    }

    public double intensity() {
        return intensity;
    }

    public long durationMillis() {
        return durationMillis;
    }

    public long startedAtMillis() {
        return startedAtMillis;
    }

    public boolean isEmpty() {
        return effectType == EffectType.NONE;
    }

    public boolean isActiveAt(long nowMillis) {
        return remainingRatioAt(nowMillis) > 0.0;
    }

    public double remainingRatioAt(long nowMillis) {
        if (isEmpty() || durationMillis == 0L) {
            return 0.0;
        }

        long safeNowMillis = Math.max(0L, nowMillis);
        long elapsedMillis = Math.max(0L, safeNowMillis - startedAtMillis);
        if (elapsedMillis >= durationMillis) {
            return 0.0;
        }
        return 1.0 - (elapsedMillis / (double) durationMillis);
    }

    public double scaledIntensityAt(long nowMillis) {
        return intensity * remainingRatioAt(nowMillis);
    }

    public enum EffectType {
        NONE("none"),
        SCREEN_SHAKE("screen_shake"),
        FOG_TINT("fog_tint"),
        TITLE_FLASH("title_flash");

        private final String wireName;

        EffectType(String wireName) {
            this.wireName = wireName;
        }

        public static EffectType fromWireName(String wireName) {
            String normalizedWireName = wireName == null ? "" : wireName.trim().toLowerCase();
            return switch (normalizedWireName) {
                case "screen_shake", "camera_shake" -> SCREEN_SHAKE;
                case "fog_tint", "fog_pulse" -> FOG_TINT;
                case "title_flash", "title" -> TITLE_FLASH;
                default -> NONE;
            };
        }

        public String wireName() {
            return wireName;
        }
    }
}
