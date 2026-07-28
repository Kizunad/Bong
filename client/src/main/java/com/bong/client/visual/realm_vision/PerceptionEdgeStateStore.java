package com.bong.client.visual.realm_vision;

import java.util.concurrent.atomic.AtomicReference;

public final class PerceptionEdgeStateStore {
    private static final AtomicReference<PerceptionEdgeState> STATE =
        new AtomicReference<>(PerceptionEdgeState.empty());

    private PerceptionEdgeStateStore() {
    }

    public static PerceptionEdgeState snapshot() {
        return STATE.get();
    }

    /** 断线时恢复视觉边缘感知的既有空初值。 */
    public static void clearOnDisconnect() {
        STATE.set(PerceptionEdgeState.empty());
    }

    public static void replace(PerceptionEdgeState state) {
        STATE.set(state == null ? PerceptionEdgeState.empty() : state);
    }
}
