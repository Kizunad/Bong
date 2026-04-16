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
            return new Snapshot(
                18f, 54f, 22f, 5f, 1f,
                "q1.0 · Mellow 0.30",
                "q0.7 · Mellow 0.50",
                "q0.4 · Turbid 0.80"
            );
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

    public static void resetForTests() {
        snapshot = Snapshot.empty();
    }
}
