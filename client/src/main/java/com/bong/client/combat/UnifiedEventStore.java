package com.bong.client.combat;

/** Singleton mutable stream. Tests reset via {@link #resetForTests()}. */
public final class UnifiedEventStore {
    private static volatile UnifiedEventStream stream = new UnifiedEventStream();

    private UnifiedEventStore() {
    }

    public static UnifiedEventStream stream() {
        return stream;
    }

    /**
     * Drops only this session's event payload while preserving the stream
     * instance held by HUD consumers and test seams.
     */
    public static void clearOnDisconnect() {
        stream.clear();
    }

    public static void resetForTests() {
        stream = new UnifiedEventStream();
    }
}
