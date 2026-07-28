package com.bong.client.hud;

import java.util.concurrent.atomic.AtomicReference;

public final class SwordBondHudStateStore {
    private static final AtomicReference<SwordBondHudState> STATE =
        new AtomicReference<>(SwordBondHudState.INACTIVE);

    private SwordBondHudStateStore() {}

    public static SwordBondHudState snapshot() {
        return STATE.get();
    }

    public static void replace(SwordBondHudState state) {
        STATE.set(state == null ? SwordBondHudState.INACTIVE : state);
    }

    public static void clear() {
        STATE.set(SwordBondHudState.INACTIVE);
    }
}
