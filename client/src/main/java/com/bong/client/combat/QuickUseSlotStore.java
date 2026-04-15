package com.bong.client.combat;

/** Client-side store for F1-F9 quick-use slot config (§11.1). */
public final class QuickUseSlotStore {
    private static volatile QuickSlotConfig snapshot = QuickSlotConfig.empty();

    private QuickUseSlotStore() {
    }

    public static QuickSlotConfig snapshot() {
        return snapshot;
    }

    public static void replace(QuickSlotConfig next) {
        snapshot = next == null ? QuickSlotConfig.empty() : next;
    }

    public static void resetForTests() {
        snapshot = QuickSlotConfig.empty();
    }
}
