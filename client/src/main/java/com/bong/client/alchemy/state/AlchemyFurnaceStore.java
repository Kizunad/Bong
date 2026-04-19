package com.bong.client.alchemy.state;

// plan-alchemy-v1 P6 — 炼丹炉快照本地 Store。
public final class AlchemyFurnaceStore {
    public record Snapshot(int tier, float integrity, float integrityMax, String ownerName) {
        public static Snapshot empty() {
            return new Snapshot(1, 92f, 100f, "self");
        }
    }

    private static volatile Snapshot snapshot = Snapshot.empty();

    private AlchemyFurnaceStore() {
    }

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
