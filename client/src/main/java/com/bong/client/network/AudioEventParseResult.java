package com.bong.client.network;

public final class AudioEventParseResult {
    private final AudioEventPayload payload;
    private final String errorMessage;

    private AudioEventParseResult(AudioEventPayload payload, String errorMessage) {
        this.payload = payload;
        this.errorMessage = errorMessage;
    }

    static AudioEventParseResult success(AudioEventPayload payload) {
        return new AudioEventParseResult(payload, null);
    }

    static AudioEventParseResult error(String errorMessage) {
        return new AudioEventParseResult(null, errorMessage);
    }

    public boolean isSuccess() {
        return payload != null;
    }

    public AudioEventPayload payload() {
        return payload;
    }

    public String errorMessage() {
        return errorMessage;
    }
}
