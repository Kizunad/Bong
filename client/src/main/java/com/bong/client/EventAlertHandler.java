package com.bong.client;

import java.util.Objects;

public final class EventAlertHandler {
    private EventAlertHandler() {
    }

    static void handle(BongServerPayload.EventAlertPayload payload) {
        handle(payload, System.currentTimeMillis());
    }

    static void handle(BongServerPayload.EventAlertPayload payload, long nowMs) {
        Objects.requireNonNull(payload, "payload");
        EventAlertState.recordAlert(payload.eventAlert(), nowMs);
    }
}
