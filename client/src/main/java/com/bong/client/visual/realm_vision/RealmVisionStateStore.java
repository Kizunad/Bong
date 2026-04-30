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

    public static void tick() {
        STATE.updateAndGet(state -> {
            if (state == null || state.isEmpty() || state.transitionTicks() <= 0) {
                return state;
            }
            int elapsed = Math.min(state.transitionTicks(), state.elapsedTicks() + 1);
            return new RealmVisionState(
                state.current(),
                state.previous(),
                state.transitionTicks(),
                elapsed,
                state.startedAtTick(),
                state.serverViewDistanceChunks()
            );
        });
    }

    public static void replace(RealmVisionState state) {
        STATE.set(state == null ? RealmVisionState.empty() : state);
    }
}
