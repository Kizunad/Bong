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

    /** Clears the server-provided unlocked-style snapshot for this session. */
    public static void clearOnDisconnect() {
        snapshot = UnlockedStyles.none();
    }

    public static void resetForTests() {
        snapshot = UnlockedStyles.none();
    }
}
