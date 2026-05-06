package com.bong.client.lingtian.state;

import java.util.Locale;

/**
 * plan-lingtian-v1 §4 / UI 切片 — 当前活跃 lingtian session 本地 Store。
 *
 * <p>Server 每 tick 推一次 {@code lingtian_session} payload；本 store 覆盖式更新。
 * {@link Snapshot#active} 为 false 表示当前无 session（HUD 应隐藏进度条）。</p>
 */
public final class LingtianSessionStore {

    public enum Kind {
        TILL,
        RENEW,
        PLANTING,
        HARVEST,
        REPLENISH,
        DRAIN_QI;

        public static Kind fromWire(String wire) {
            if (wire == null) return TILL;
            return switch (wire.toLowerCase(Locale.ROOT)) {
                case "till" -> TILL;
                case "renew" -> RENEW;
                case "planting" -> PLANTING;
                case "harvest" -> HARVEST;
                case "replenish" -> REPLENISH;
                case "drain_qi" -> DRAIN_QI;
                default -> TILL;
            };
        }

        /** UI 标签（中文）。 */
        public String label() {
            return switch (this) {
                case TILL -> "开垦";
                case RENEW -> "翻新";
                case PLANTING -> "种植";
                case HARVEST -> "收获";
                case REPLENISH -> "补灵";
                case DRAIN_QI -> "吸灵";
            };
        }
    }

    public record Snapshot(
        boolean active,
        Kind kind,
        int x,
        int y,
        int z,
        int elapsedTicks,
        int targetTicks,
        String plantId,
        String source,
        float dyeContamination,
        boolean dyeContaminationWarning
    ) {
        public static Snapshot empty() {
            return new Snapshot(false, Kind.TILL, 0, 0, 0, 0, 0, null, null, 0.0f, false);
        }

        public float progress() {
            if (targetTicks <= 0) return 0.0f;
            float p = (float) elapsedTicks / (float) targetTicks;
            return Math.max(0.0f, Math.min(1.0f, p));
        }
    }

    private static volatile Snapshot snapshot = Snapshot.empty();

    private LingtianSessionStore() {}

    public static Snapshot snapshot() {
        return snapshot;
    }

    public static void replace(Snapshot s) {
        snapshot = s;
    }
}
