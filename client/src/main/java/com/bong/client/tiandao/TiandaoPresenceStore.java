package com.bong.client.tiandao;

import java.util.concurrent.atomic.AtomicReference;

public final class TiandaoPresenceStore {
    private static final AtomicReference<TiandaoPresenceState> STATE =
        new AtomicReference<>(TiandaoPresenceState.empty());

    private TiandaoPresenceStore() {
    }

    public static TiandaoPresenceState snapshot() {
        return STATE.get();
    }

    public static void replace(TiandaoPresenceState state) {
        STATE.set(state == null ? TiandaoPresenceState.empty() : state);
    }

    public static void clear() {
        STATE.set(TiandaoPresenceState.empty());
    }
}
