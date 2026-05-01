package com.bong.client.state;

public final class RealmCollapseHudStateStore {
    private static volatile RealmCollapseHudState snapshot = RealmCollapseHudState.empty();

    private RealmCollapseHudStateStore() {
    }

    public static RealmCollapseHudState snapshot() {
        return snapshot;
    }

    public static void replace(RealmCollapseHudState next) {
        snapshot = next == null ? RealmCollapseHudState.empty() : next;
    }

    public static void resetForTests() {
        snapshot = RealmCollapseHudState.empty();
    }
}
