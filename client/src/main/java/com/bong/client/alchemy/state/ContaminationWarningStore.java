package com.bong.client.alchemy.state;

// plan-alchemy-v1 P6 — 丹毒预警快照本地 Store。
public final class ContaminationWarningStore {
    public record Snapshot(
        float mellowCurrent,
        float mellowMax,
        boolean mellowOk,
        float violentCurrent,
        float violentMax,
        boolean violentOk,
        String metabolismNote
    ) {
        public static Snapshot empty() {
            return new Snapshot(
                0.18f, 0.60f, true,
                0.93f, 1.00f, false,
                "代谢速率 = 经脉 sum_rate × integrity（contamination_tick 10:15）"
            );
        }
    }

    private static volatile Snapshot snapshot = Snapshot.empty();

    private ContaminationWarningStore() {
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
