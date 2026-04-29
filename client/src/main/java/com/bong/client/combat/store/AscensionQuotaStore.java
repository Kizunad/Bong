package com.bong.client.combat.store;

/**
 * Server-wide ascension quota snapshot. Last-write-wins.
 */
public final class AscensionQuotaStore {
    public record State(
        int occupiedSlots,
        int quotaLimit,
        int availableSlots
    ) {
        public State {
            occupiedSlots = Math.max(0, occupiedSlots);
            quotaLimit = Math.max(0, quotaLimit);
            availableSlots = Math.max(0, availableSlots);
        }

        public static final State EMPTY = new State(0, 0, 0);
    }

    private static volatile State snapshot = State.EMPTY;

    private AscensionQuotaStore() {}

    public static State snapshot() { return snapshot; }

    public static void replace(State next) {
        snapshot = next == null ? State.EMPTY : next;
    }

    public static void resetForTests() { snapshot = State.EMPTY; }
}
