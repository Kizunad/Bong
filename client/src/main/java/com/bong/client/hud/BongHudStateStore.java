package com.bong.client.hud;

public final class BongHudStateStore {
    private static volatile BongHudStateSnapshot snapshot = BongHudStateSnapshot.empty();

    private BongHudStateStore() {
    }

    public static BongHudStateSnapshot snapshot() {
        return snapshot;
    }

    public static void replace(BongHudStateSnapshot nextSnapshot) {
        snapshot = nextSnapshot == null ? BongHudStateSnapshot.empty() : nextSnapshot;
    }
}
