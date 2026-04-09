package com.bong.client.network;

public final class ServerPayloadParseResult {
    private final boolean success;
    private final ServerDataEnvelope envelope;
    private final String errorMessage;

    private ServerPayloadParseResult(boolean success, ServerDataEnvelope envelope, String errorMessage) {
        this.success = success;
        this.envelope = envelope;
        this.errorMessage = errorMessage;
    }

    public static ServerPayloadParseResult success(ServerDataEnvelope envelope) {
        return new ServerPayloadParseResult(true, envelope, null);
    }

    public static ServerPayloadParseResult error(String errorMessage) {
        return new ServerPayloadParseResult(false, null, errorMessage);
    }

    public boolean isSuccess() {
        return success;
    }

    public ServerDataEnvelope envelope() {
        return envelope;
    }

    public String errorMessage() {
        return errorMessage;
    }
}
