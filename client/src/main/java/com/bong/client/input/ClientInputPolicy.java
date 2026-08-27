package com.bong.client.input;

import com.bong.client.tsy.ExtractStateStore;

/** Cross-domain input arbitration shared by client feature entry points. */
public final class ClientInputPolicy {
    private ClientInputPolicy() {
    }

    /**
     * The legacy Forge U binding may consume a queued press only when extraction
     * is idle; extraction owns that physical input while it is active.
     */
    public static boolean shouldDispatchForgeOpen() {
        return !ExtractStateStore.snapshot().extracting();
    }
}
