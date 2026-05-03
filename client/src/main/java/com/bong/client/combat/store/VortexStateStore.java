package com.bong.client.combat.store;

/** Authoritative woliu vortex HUD state from {@code vortex_state}. */
public final class VortexStateStore {
    public record State(
        boolean active,
        float radius,
        float delta,
        float envQiAtCast,
        long maintainRemainingTicks,
        int interceptedCount
    ) {
        public static final State NONE = new State(false, 0f, 0f, 0f, 0L, 0);
    }

    private static volatile State snapshot = State.NONE;

    private VortexStateStore() {}

    public static State snapshot() { return snapshot; }

    public static void replace(State next) {
        snapshot = next == null ? State.NONE : next;
    }

    public static void resetForTests() {
        snapshot = State.NONE;
    }
}
