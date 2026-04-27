package com.bong.client.forge.state;

/** plan-forge-v1 §4 — 锻造结果结算本地 Store。 */
public final class ForgeOutcomeStore {
    public record Snapshot(long sessionId, String blueprintId, String bucket,
                           String weaponItem, float quality, String colorName,
                           String sideEffectsCsv, int achievedTier, boolean flawedPath) {
        public static Snapshot empty() {
            return new Snapshot(0, "", "waste", null, 0f, null, "", 0, false);
        }
    }

    private static volatile Snapshot lastOutcome = Snapshot.empty();
    private static volatile Snapshot displayedOutcome = Snapshot.empty();

    private ForgeOutcomeStore() {}

    public static Snapshot lastOutcome() {
        return lastOutcome;
    }

    public static Snapshot displayedOutcome() {
        return displayedOutcome;
    }

    public static void replace(Snapshot next) {
        lastOutcome = next == null ? Snapshot.empty() : next;
    }

    /** Mark the current outcome as displayed (UI dismissed). */
    public static void markDisplayed() {
        displayedOutcome = lastOutcome;
    }

    public static boolean hasNewOutcome() {
        Snapshot a = lastOutcome;
        Snapshot b = displayedOutcome;
        return a.sessionId != b.sessionId || !a.bucket.equals(b.bucket);
    }

    public static void resetForTests() {
        lastOutcome = Snapshot.empty();
        displayedOutcome = Snapshot.empty();
    }
}
