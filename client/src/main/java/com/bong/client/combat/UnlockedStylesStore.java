package com.bong.client.combat;

public final class UnlockedStylesStore {
    private static volatile UnlockedStyles snapshot = UnlockedStyles.none();

    private UnlockedStylesStore() {
    }

    public static UnlockedStyles snapshot() {
        return snapshot;
    }

    public static void replace(UnlockedStyles next) {
        snapshot = next == null ? UnlockedStyles.none() : next;
    }

    public static void resetForTests() {
        snapshot = UnlockedStyles.none();
    }
}
