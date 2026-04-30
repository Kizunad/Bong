package com.bong.client.visual.realm_vision;

import java.util.concurrent.atomic.AtomicReference;

public final class RealmVisionStateStore {
    private static final AtomicReference<RealmVisionState> STATE =
        new AtomicReference<>(RealmVisionState.empty());

    private RealmVisionStateStore() {
    }

    public static RealmVisionState snapshot() {
        return STATE.get();
    }

    public static void replace(RealmVisionState state) {
        STATE.set(state == null ? RealmVisionState.empty() : state);
    }
}
