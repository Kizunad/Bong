package com.bong.client.network.handlers;

import com.bong.client.BongClient;
import com.bong.client.network.PayloadHandler;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParseException;
import com.google.gson.JsonParser;
import net.minecraft.client.MinecraftClient;

public class UiOpenHandler implements PayloadHandler {
    private static final int MAX_XML_LENGTH = 10_000;

    @Override
    public void handle(MinecraftClient client, String type, String jsonPayload) {
        handlePayload(jsonPayload);
    }

    void handlePayload(String jsonPayload) {
        ParseResult result = parse(jsonPayload);
        if (!result.success()) {
            BongClient.LOGGER.warn("Ignoring malformed ui_open payload: {}", result.errorMessage());
            return;
        }

        if (result.ui() == null) {
            BongClient.LOGGER.debug("Ignoring optional ui_open payload; static cultivation screen path remains active");
        } else {
            BongClient.LOGGER.debug(
                "Ignoring optional ui_open payload for ui `{}`; static cultivation screen path remains active",
                result.ui()
            );
        }
    }

    private ParseResult parse(String jsonPayload) {
        try {
            JsonElement rootElement = JsonParser.parseString(jsonPayload);
            if (!rootElement.isJsonObject()) {
                return ParseResult.error("ui_open root must be an object");
            }

            JsonObject rootObject = rootElement.getAsJsonObject();
            String xml = getRequiredString(rootObject, "xml");
            if (xml == null || xml.isBlank()) {
                return ParseResult.error("ui_open missing required string field 'xml'");
            }
            if (xml.length() > MAX_XML_LENGTH) {
                return ParseResult.error("ui_open xml exceeds size budget");
            }

            return ParseResult.success(getOptionalString(rootObject, "ui"), xml);
        } catch (JsonParseException | IllegalStateException exception) {
            return ParseResult.error("Malformed ui_open payload");
        }
    }

    private static String getRequiredString(JsonObject object, String key) {
        JsonElement value = object.get(key);
        if (value == null || !value.isJsonPrimitive() || !value.getAsJsonPrimitive().isString()) {
            return null;
        }

        return value.getAsString();
    }

    private static String getOptionalString(JsonObject object, String key) {
        JsonElement value = object.get(key);
        if (value == null || value.isJsonNull()) {
            return null;
        }
        if (!value.isJsonPrimitive() || !value.getAsJsonPrimitive().isString()) {
            return null;
        }

        String parsed = value.getAsString();
        return parsed.isBlank() ? null : parsed;
    }

    private record ParseResult(boolean success, String ui, String xml, String errorMessage) {
        private static ParseResult success(String ui, String xml) {
            return new ParseResult(true, ui, xml, null);
        }

        private static ParseResult error(String errorMessage) {
            return new ParseResult(false, null, null, errorMessage);
        }
    }
}
