package com.bong.client.combat.store;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

/**
 * Server-wide ascension quota snapshot. Last-write-wins.
 */
public final class AscensionQuotaStore {
    public record State(
        int occupiedSlots,
        int quotaLimit,
        int availableSlots,
        double totalWorldQi,
        double quotaK,
        String quotaBasis
    ) {
        public State {
            occupiedSlots = Math.max(0, occupiedSlots);
            quotaLimit = Math.max(0, quotaLimit);
            availableSlots = Math.max(0, availableSlots);
            totalWorldQi = Double.isFinite(totalWorldQi) ? Math.max(0.0, totalWorldQi) : 0.0;
            quotaK = Double.isFinite(quotaK) ? Math.max(0.0, quotaK) : 0.0;
            quotaBasis = quotaBasis == null ? "" : quotaBasis;
        }

        public static final State EMPTY = new State(0, 0, 0, 0.0, 0.0, "");
    }

    private static volatile State snapshot = State.EMPTY;
    private static final List<Consumer<State>> listeners = new CopyOnWriteArrayList<>();

    private AscensionQuotaStore() {}

    public static State snapshot() { return snapshot; }

    public static void replace(State next) {
        State normalized = next == null ? State.EMPTY : next;
        snapshot = normalized;
        for (Consumer<State> listener : listeners) {
            listener.accept(normalized);
        }
    }

    public static void addListener(Consumer<State> listener) {
        listeners.add(listener);
    }

    public static void removeListener(Consumer<State> listener) {
        listeners.remove(listener);
    }

    public static void resetForTests() {
        snapshot = State.EMPTY;
        listeners.clear();
    }
}
