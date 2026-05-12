package com.bong.client.movement;

import com.bong.client.network.ClientRequestProtocol;

public final class MovementKeyRouter {
    public ClientRequestProtocol.MovementAction route(
        boolean dashTapped,
        boolean slideTapped,
        boolean jumpTapped,
        boolean airborne
    ) {
        if (jumpTapped && airborne) {
            return ClientRequestProtocol.MovementAction.DOUBLE_JUMP;
        }
        if (slideTapped) {
            return ClientRequestProtocol.MovementAction.SLIDE;
        }
        if (dashTapped) {
            return ClientRequestProtocol.MovementAction.DASH;
        }
        return null;
    }

    public void reset() {
    }
}
