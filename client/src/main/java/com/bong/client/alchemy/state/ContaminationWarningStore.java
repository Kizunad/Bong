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
            return neutral();
        }

        public static Snapshot neutral() {
            return new Snapshot(0.0f, 0.0f, true, 0.0f, 0.0f, true, "");
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

    public static void clearOnDisconnect() {
        replace(Snapshot.neutral());
    }

    public static void resetForTests() {
        snapshot = Snapshot.empty();
    }
}
