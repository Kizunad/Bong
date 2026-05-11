package com.bong.client.tsy;

public final class TsyBossHealthStore {
    private static volatile TsyBossHealthState state = TsyBossHealthState.empty();

    private TsyBossHealthStore() {
    }

    public static TsyBossHealthState snapshot() {
        return state;
    }

    public static void replace(TsyBossHealthState next) {
        state = next == null ? TsyBossHealthState.empty() : next;
    }

    public static void resetForTests() {
        state = TsyBossHealthState.empty();
    }
}
