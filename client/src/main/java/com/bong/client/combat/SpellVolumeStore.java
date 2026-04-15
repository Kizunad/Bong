package com.bong.client.combat;

public final class SpellVolumeStore {
    private static volatile SpellVolumeState snapshot = SpellVolumeState.idle();

    private SpellVolumeStore() {
    }

    public static SpellVolumeState snapshot() {
        return snapshot;
    }

    public static void replace(SpellVolumeState next) {
        snapshot = next == null ? SpellVolumeState.idle() : next;
    }

    public static void show(float radius, float velocityCap, float qiInvest) {
        snapshot = SpellVolumeState.visible(radius, velocityCap, qiInvest);
    }

    public static void hide() {
        snapshot = snapshot.hidden();
    }

    public static void resetForTests() {
        snapshot = SpellVolumeState.idle();
    }
}
