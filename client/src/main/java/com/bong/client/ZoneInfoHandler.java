package com.bong.client;

import java.util.Objects;

public final class ZoneInfoHandler {
    private ZoneInfoHandler() {
    }

    static void handle(BongServerPayload.ZoneInfoPayload payload) {
        handle(payload, System.currentTimeMillis());
    }

    static void handle(BongServerPayload.ZoneInfoPayload payload, long nowMs) {
        Objects.requireNonNull(payload, "payload");
        ZoneState.recordZoneInfo(payload.zoneInfo(), nowMs);
    }
}
