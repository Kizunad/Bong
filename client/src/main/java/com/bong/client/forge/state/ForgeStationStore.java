package com.bong.client.forge.state;

/** plan-forge-v1 §4 — 锻炉快照本地 Store。 */
public final class ForgeStationStore {
    public record Snapshot(String stationId, int tier, float integrity, String ownerName,
                           boolean hasSession) {
        public static Snapshot empty() {
            return new Snapshot("", 1, 1.0f, "", false);
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

    public static void resetForTests() {
        snapshot = Snapshot.empty();
    }
}
