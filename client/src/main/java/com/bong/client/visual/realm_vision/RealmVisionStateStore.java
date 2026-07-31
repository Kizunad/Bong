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

    /** 断线时恢复境界视界的既有空初值。 */
    public static void clearOnDisconnect() {
        STATE.set(RealmVisionState.empty());
    }

    public static void replace(RealmVisionState state) {
        STATE.set(state == null ? RealmVisionState.empty() : state);
    }
}
