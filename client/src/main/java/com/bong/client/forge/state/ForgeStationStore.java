package com.bong.client.forge.state;

import net.minecraft.util.math.BlockPos;

/** plan-forge-v1 §4 — 锻炉快照本地 Store。 */
public final class ForgeStationStore {
    /**
     * 服务端确认的工位身份；pos 未就绪时不能开启工位窗口或发送起炉请求。
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
