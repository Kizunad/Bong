package com.bong.client.state;

public final class PlayerStateStore {
    private static volatile PlayerStateViewModel snapshot = PlayerStateViewModel.empty();

    private PlayerStateStore() {
    }

    public static PlayerStateViewModel snapshot() {
        return snapshot;
    }

    public static void replace(PlayerStateViewModel nextSnapshot) {
        snapshot = nextSnapshot == null ? PlayerStateViewModel.empty() : nextSnapshot;
    }

    public static void resetForTests() {
        snapshot = PlayerStateViewModel.empty();
    }
}
