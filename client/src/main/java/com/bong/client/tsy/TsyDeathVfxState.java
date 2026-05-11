package com.bong.client.tsy;

public record TsyDeathVfxState(boolean active, long startedAtMillis) {
    public static TsyDeathVfxState empty() {
        return new TsyDeathVfxState(false, 0L);
    }

    public boolean activeAt(long nowMillis) {
        return active && Math.max(0L, nowMillis - startedAtMillis) < 1_000L;
    }
}
