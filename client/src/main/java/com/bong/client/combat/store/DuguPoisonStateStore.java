package com.bong.client.combat.store;

/** Authoritative self-visible poison HUD state from {@code dugu_poison_state}. */
public final class DuguPoisonStateStore {
    public record State(
        boolean active,
        String meridianId,
        String attacker,
        long attachedAtTick,
        int poisonerRealmTier,
        double lossPerTick,
        double flowCapacityAfter,
        double qiMaxAfter,
        long serverTick
    ) {
        public static final State NONE = new State(false, "", "", 0L, 0, 0d, 0d, 0d, 0L);
    }

    private static volatile State snapshot = State.NONE;

    private DuguPoisonStateStore() {}

    public static State snapshot() { return snapshot; }

    public static void replace(State next) {
        snapshot = next == null ? State.NONE : next;
    }

    public static void resetForTests() {
        snapshot = State.NONE;
    }
}
