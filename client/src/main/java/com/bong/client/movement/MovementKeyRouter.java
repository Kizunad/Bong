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

    public void reset() {
    }
}
