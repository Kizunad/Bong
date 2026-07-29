package com.bong.client.forge.state;

import net.minecraft.util.math.BlockPos;

/** plan-forge-v1 §4 — 锻炉快照本地 Store。 */
public final class ForgeStationStore {
    /**
     * plan-forge-session-entry-wiring-v1 §4.1#3（新发现连带缺口）—— {@code pos} 供 U 键
     * 全局打开、无 station 交互上下文的 {@code ForgeScreen} 在起炉时知道往哪个坐标发
     * {@code ForgeStartSession.station_pos}。收到第一条 {@code forge_station} 快照前
     * {@code pos = null}（尚不知道玩家的砧在哪，起炉入口应保持不可用）。
     */
    public record Snapshot(BlockPos pos, String stationId, int tier, float integrity, String ownerName,
                           boolean hasSession) {
        public static Snapshot empty() {
            return new Snapshot(null, "", 1, 1.0f, "", false);
        }
    }

    private static volatile Snapshot snapshot = Snapshot.empty();

    private ForgeStationStore() {}

    public static Snapshot snapshot() {
        return snapshot;
    }

    public static void replace(Snapshot next) {
        snapshot = next == null ? Snapshot.empty() : next;
    }

    public static void clearOnDisconnect() {
        replace(null);
    }

    public static void resetForTests() {
        snapshot = Snapshot.empty();
    }
}
