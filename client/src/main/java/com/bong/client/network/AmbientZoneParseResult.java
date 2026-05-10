package com.bong.client.network;

public final class AmbientZoneParseResult {
    private final AmbientZonePayload payload;
    private final String errorMessage;

    private AmbientZoneParseResult(AmbientZonePayload payload, String errorMessage) {
        this.payload = payload;
        this.errorMessage = errorMessage;
    }

    static AmbientZoneParseResult success(AmbientZonePayload payload) {
        return new AmbientZoneParseResult(payload, null);
    }

    static AmbientZoneParseResult error(String errorMessage) {
        return new AmbientZoneParseResult(null, errorMessage);
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
