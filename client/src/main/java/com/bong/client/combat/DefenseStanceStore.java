package com.bong.client.combat;

public final class DefenseStanceStore {
    private static volatile DefenseStanceState snapshot = DefenseStanceState.none();

    private DefenseStanceStore() {
    }

    public static DefenseStanceState snapshot() {
        return snapshot;
    }

    public static void replace(DefenseStanceState next) {
        snapshot = next == null ? DefenseStanceState.none() : next;
    }

    public static void resetForTests() {
        snapshot = DefenseStanceState.none();
    }
}
