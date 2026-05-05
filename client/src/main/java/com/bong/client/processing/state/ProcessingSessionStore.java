package com.bong.client.processing.state;

import java.util.Locale;

/** plan-lingtian-process-v1 P3 — 当前加工 session 的本地覆盖式 store。 */
public final class ProcessingSessionStore {
    public enum Kind {
        DRYING,
        GRINDING,
        FORGING_ALCHEMY,
        EXTRACTION;

        public static Kind fromWire(String wire) {
            if (wire == null) return DRYING;
            return switch (wire.toLowerCase(Locale.ROOT)) {
                case "drying" -> DRYING;
                case "grinding" -> GRINDING;
                case "forging_alchemy" -> FORGING_ALCHEMY;
                case "extraction" -> EXTRACTION;
                default -> DRYING;
            };
        }

        public String label() {
            return switch (this) {
                case DRYING -> "晾晒";
                case GRINDING -> "碾粉";
                case FORGING_ALCHEMY -> "炮制";
                case EXTRACTION -> "萃取";
            };
        }
    }

    public record Snapshot(
        boolean active,
        String sessionId,
        Kind kind,
        String recipeId,
        int progressTicks,
        int durationTicks,
        String playerId
    ) {
        public static Snapshot empty() {
            return new Snapshot(false, "", Kind.DRYING, "", 0, 0, "");
        }

        public float progress() {
            if (durationTicks <= 0) return 0.0f;
            float p = (float) progressTicks / (float) durationTicks;
            return Math.max(0.0f, Math.min(1.0f, p));
        }
    }

    private static volatile Snapshot snapshot = Snapshot.empty();

    private ProcessingSessionStore() {}

    public static Snapshot snapshot() {
        return snapshot;
    }

    public static void replace(Snapshot next) {
        snapshot = next == null ? Snapshot.empty() : next;
    }

    public static void resetForTests() {
        snapshot = Snapshot.empty();
    }
}
