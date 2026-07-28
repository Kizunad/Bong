package com.bong.client.movement;

import com.bong.client.network.ClientRequestProtocol;

public final class MovementKeyRouter {
    public ClientRequestProtocol.MovementAction route(
        boolean dashTapped
    ) {
        if (dashTapped) {
            return ClientRequestProtocol.MovementAction.DASH;
        }
        return null;
    }

    /**
     * Resets session-bound routing adjunct state without touching registry-owned stores.
     *
     * <p>The router is currently stateless, but this explicit production boundary keeps
     * future per-session input routing state out of the central Store registry.</p>
     */
    public void resetOnDisconnect() {
    }
}
