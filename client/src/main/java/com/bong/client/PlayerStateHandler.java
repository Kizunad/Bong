package com.bong.client;

import java.util.Objects;

public final class PlayerStateHandler {
    private PlayerStateHandler() {
    }

    static void handle(BongServerPayload.PlayerStatePayload payload) {
        handle(payload, System.currentTimeMillis());
    }

    static void handle(BongServerPayload.PlayerStatePayload payload, long nowMs) {
        Objects.requireNonNull(payload, "payload");
        PlayerStateState.record(payload.playerState(), nowMs);
    }
}
