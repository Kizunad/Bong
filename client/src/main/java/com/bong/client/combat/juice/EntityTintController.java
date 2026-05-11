package com.bong.client.combat.juice;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

public final class EntityTintController {
    private static final Map<String, Tint> TINTS = new ConcurrentHashMap<>();

    private EntityTintController() {
    }

    public static Tint trigger(String entityUuid, CombatJuiceProfile profile, long nowMs) {
        if (entityUuid == null || entityUuid.isBlank() || profile == null || profile.tintDurationTicks() <= 0) {
            return Tint.none();
        }
        Tint tint = new Tint(entityUuid, profile.qiColorArgb(), 0.4f, profile.tintDurationTicks(), nowMs);
        TINTS.put(entityUuid, tint);
        return tint;
    }

    public static Tint activeTint(String entityUuid, long nowMs) {
        if (entityUuid == null || entityUuid.isBlank()) {
            return Tint.none();
        }
        Tint tint = TINTS.get(entityUuid);
        if (tint == null || !tint.activeAt(nowMs)) {
            if (tint != null) {
                TINTS.remove(entityUuid, tint);
            }
            return Tint.none();
        }
        return tint;
    }

    public static void tick(long nowMs) {
        for (Map.Entry<String, Tint> entry : TINTS.entrySet()) {
            if (!entry.getValue().activeAt(nowMs)) {
                TINTS.remove(entry.getKey(), entry.getValue());
            }
        }
    }

    public static void resetForTests() {
        TINTS.clear();
    }

    public record Tint(String entityUuid, int argb, float maxAlpha, int durationTicks, long startedAtMs) {
        public static Tint none() {
            return new Tint("", 0, 0f, 0, 0L);
        }

        public long durationMillis() {
            return Math.max(0, durationTicks) * 50L;
        }

        public boolean activeAt(long nowMs) {
            return !entityUuid.isBlank() && alphaAt(nowMs) > 0f;
        }

        public float alphaAt(long nowMs) {
            long duration = durationMillis();
            if (duration <= 0L) {
                return 0f;
            }
            long elapsed = Math.max(0L, nowMs - startedAtMs);
            if (elapsed >= duration) {
                return 0f;
            }
            return maxAlpha * (1.0f - elapsed / (float) duration);
        }

        public int colorAt(long nowMs) {
            int alpha = Math.max(0, Math.min(255, Math.round(alphaAt(nowMs) * 255f)));
            return (alpha << 24) | (argb & 0x00FFFFFF);
        }
    }
}
