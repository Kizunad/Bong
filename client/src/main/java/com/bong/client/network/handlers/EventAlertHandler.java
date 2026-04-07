package com.bong.client.network.handlers;

import com.bong.client.BongClient;
import com.bong.client.EventAlertState;
import com.bong.client.network.PayloadHandler;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParseException;
import com.google.gson.JsonParser;
import net.minecraft.client.MinecraftClient;

public class EventAlertHandler implements PayloadHandler {
    @Override
    public void handle(MinecraftClient client, String type, String jsonPayload) {
        handlePayload(jsonPayload);
    }

    void handlePayload(String jsonPayload) {
        ParseResult result = parse(jsonPayload);
        if (!result.success()) {
            BongClient.LOGGER.warn("Ignoring malformed event_alert payload: {}", result.errorMessage());
            return;
        }

        EventAlertState.show(result.message());
    }

    private ParseResult parse(String jsonPayload) {
        try {
            JsonElement rootElement = JsonParser.parseString(jsonPayload);
            if (!rootElement.isJsonObject()) {
                return ParseResult.error("Event alert root must be an object");
            }

            JsonObject rootObject = rootElement.getAsJsonObject();
            String event = getRequiredString(rootObject, "event");
            if (!isSupportedEvent(event)) {
                return ParseResult.error("Event alert has unsupported or missing 'event'");
            }

            String message = getRequiredString(rootObject, "message");
            if (message == null || message.isBlank()) {
                return ParseResult.error("Event alert missing required string field 'message'");
            }

            return ParseResult.success(message);
        } catch (JsonParseException | IllegalStateException exception) {
            return ParseResult.error("Malformed event alert payload");
        }
    }

    private static String getRequiredString(JsonObject object, String key) {
        JsonElement value = object.get(key);
        if (value == null || !value.isJsonPrimitive() || !value.getAsJsonPrimitive().isString()) {
            return null;
        }

        return value.getAsString();
    }

    private static boolean isSupportedEvent(String event) {
        if (event == null) {
            return false;
        }

        return event.equals("thunder_tribulation") || event.equals("beast_tide");
    }

    private record ParseResult(boolean success, String message, String errorMessage) {
        private static ParseResult success(String message) {
            return new ParseResult(true, message, null);
        }

        private static ParseResult error(String errorMessage) {
            return new ParseResult(false, null, errorMessage);
        }
    }
}
