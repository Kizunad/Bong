package com.bong.client.network;

import java.util.Objects;

public final class AmbientZoneParseResult {
    private final AmbientZonePayload payload;
    private final String errorMessage;

    private AmbientZoneParseResult(AmbientZonePayload payload, String errorMessage) {
        this.payload = payload;
        this.errorMessage = errorMessage;
    }

    static AmbientZoneParseResult success(AmbientZonePayload payload) {
        return new AmbientZoneParseResult(Objects.requireNonNull(payload, "payload"), null);
    }

    static AmbientZoneParseResult error(String errorMessage) {
        return new AmbientZoneParseResult(null, Objects.requireNonNull(errorMessage, "errorMessage"));
    }

    public boolean isSuccess() {
        return payload != null;
    }

    public AmbientZonePayload payload() {
        return payload;
    }

    public String errorMessage() {
        return errorMessage;
    }
}
