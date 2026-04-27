package com.bong.client.forge.state;

/** plan-forge-v1 §4 — 锻造会话实时状态本地 Store。 */
public final class ForgeSessionStore {
    public record Snapshot(long sessionId, String blueprintId, String blueprintName,
                           boolean active, String currentStep, int stepIndex,
                           int achievedTier, String stepStateJson) {
        public static Snapshot empty() {
            return new Snapshot(0, "", "", false, "done", 0, 0, "{}");
        }
    }

    private static volatile Snapshot snapshot = Snapshot.empty();

    private ForgeSessionStore() {}

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
