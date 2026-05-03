package com.bong.client.combat.store;

/** Authoritative anqi carrier HUD state from {@code carrier_state}. */
public final class CarrierStateStore {
    public enum Phase {
        IDLE,
        CHARGING,
        CHARGED
    }

    public record State(
        Phase phase,
        float progress,
        float sealedQi,
        float sealedQiInitial,
        long halfLifeRemainingTicks,
        long itemInstanceId
    ) {
        public static final State NONE = new State(Phase.IDLE, 0f, 0f, 0f, 0L, -1L);

        public boolean active() {
            return phase != Phase.IDLE;
        }
    }

    private static volatile State snapshot = State.NONE;

    private CarrierStateStore() {}

    public static State snapshot() {
        return snapshot;
    }

    public static void replace(State next) {
        snapshot = next == null ? State.NONE : next;
    }

    public static void resetForTests() {
        snapshot = State.NONE;
    }
}
