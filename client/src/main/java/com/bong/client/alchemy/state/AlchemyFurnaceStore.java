package com.bong.client.alchemy.state;

import net.minecraft.util.math.BlockPos;

// plan-alchemy-v1 P6 — 炼丹炉快照本地 Store。
public final class AlchemyFurnaceStore {
    public record Snapshot(BlockPos pos, int tier, float integrity, float integrityMax, String ownerName, boolean hasSession) {
        public static Snapshot empty() {
            return new Snapshot(null, 1, 92f, 100f, "self", false);
        }
    }

    private static volatile Snapshot snapshot = Snapshot.empty();

    private AlchemyFurnaceStore() {
    }

    public static Snapshot snapshot() {
        return snapshot;
    }

    public static void replace(Snapshot next) {
        Snapshot replacement = next == null ? Snapshot.empty() : next;
        Snapshot previous = snapshot;
        snapshot = replacement;
        if (!java.util.Objects.equals(previous.pos(), replacement.pos())) {
            // 炉位变化会使上一炉的会话投影失效。状态层维护这个不变量，网络层无需反向依赖 UI。
            AlchemySessionStore.clearForFurnaceChange();
        }
    }

    public static void clearOnDisconnect() {
        replace(null);
    }

    public static void resetForTests() {
        snapshot = Snapshot.empty();
    }
}
