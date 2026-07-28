package com.bong.client.alchemy.state;

// plan-alchemy-v1 P6 — 实时结果预测分桶快照本地 Store。
public final class AlchemyOutcomeForecastStore {
    public record Snapshot(
        float perfectPct,
        float goodPct,
        float flawedPct,
        float wastePct,
        float explodePct,
        String perfectNote,
        String goodNote,
        String flawedNote
    ) {
        public static Snapshot empty() {
            return neutral();
        }

        public static Snapshot neutral() {
            return new Snapshot(0f, 0f, 0f, 0f, 0f, "", "", "");
        }
    }

    private static volatile Snapshot snapshot = Snapshot.empty();

    private AlchemyOutcomeForecastStore() {
    }

    public static Snapshot snapshot() {
        return snapshot;
    }

    public static void replace(Snapshot next) {
        snapshot = next == null ? Snapshot.empty() : next;
    }

    public static void clearOnDisconnect() {
        replace(Snapshot.neutral());
    }

    public static void resetForTests() {
        snapshot = Snapshot.empty();
    }
}
