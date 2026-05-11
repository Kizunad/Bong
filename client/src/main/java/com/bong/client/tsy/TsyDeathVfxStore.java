package com.bong.client.tsy;

public final class TsyDeathVfxStore {
    private static volatile TsyDeathVfxState state = TsyDeathVfxState.empty();

    private TsyDeathVfxStore() {
    }

    public static TsyDeathVfxState snapshot() {
        return state;
    }

    public static void trigger(long nowMillis) {
        state = new TsyDeathVfxState(true, Math.max(0L, nowMillis));
    }

    public static void resetForTests() {
        state = TsyDeathVfxState.empty();
    }
}
