package com.bong.client.movement;

import com.bong.client.network.ClientRequestProtocol;

public final class MovementKeyRouter {
    public static final long DASH_DOUBLE_TAP_WINDOW_MS = 250L;

    private long lastForwardTapMs = Long.MIN_VALUE;

    public ClientRequestProtocol.MovementAction route(
        boolean forwardPressed,
        boolean forwardTapped,
        boolean sneakTapped,
        boolean sprintTapped,
        boolean jumpTapped,
        boolean airborne,
        long nowMs
    ) {
        if (jumpTapped && airborne) {
            return ClientRequestProtocol.MovementAction.DOUBLE_JUMP;
        }
        if (sprintTapped && forwardPressed) {
            return ClientRequestProtocol.MovementAction.SLIDE;
        }
        if (sneakTapped && forwardPressed) {
            lastForwardTapMs = Long.MIN_VALUE;
            return ClientRequestProtocol.MovementAction.DASH;
        }
        if (forwardTapped) {
            if (lastForwardTapMs != Long.MIN_VALUE
                && nowMs >= lastForwardTapMs
                && nowMs - lastForwardTapMs <= DASH_DOUBLE_TAP_WINDOW_MS) {
                lastForwardTapMs = Long.MIN_VALUE;
                return ClientRequestProtocol.MovementAction.DASH;
            }
            lastForwardTapMs = nowMs;
        }
        return null;
    }

    public void reset() {
        lastForwardTapMs = Long.MIN_VALUE;
    }
}
