package com.bong.client.combat;

/** Singleton mutable stream. Tests reset via {@link #resetForTests()}. */
public final class UnifiedEventStore {
    private static volatile UnifiedEventStream stream = new UnifiedEventStream();

    private UnifiedEventStore() {
    }

    public static UnifiedEventStream stream() {
        return stream;
    }

    public static void resetForTests() {
        stream = new UnifiedEventStream();
    }
}
