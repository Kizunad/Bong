package com.bong.client.hud;

import java.util.concurrent.atomic.AtomicReference;

public final class AnqiHudStateStore {
    private static final AtomicReference<AnqiHudState> STATE = new AtomicReference<>(AnqiHudState.empty());

    private AnqiHudStateStore() {}

    public static AnqiHudState snapshot() {
        return STATE.get();
    }

    public static void replace(AnqiHudState state) {
        STATE.set(state == null ? AnqiHudState.empty() : state);
    }

    public static void clear() {
        STATE.set(AnqiHudState.empty());
    }
}
