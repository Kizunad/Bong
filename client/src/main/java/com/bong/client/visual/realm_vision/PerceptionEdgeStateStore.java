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

    public static void replace(PerceptionEdgeState state) {
        STATE.set(state == null ? PerceptionEdgeState.empty() : state);
    }
}
