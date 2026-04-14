package com.bong.client.network;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.JsonPrimitive;

import java.nio.charset.StandardCharsets;
import java.util.Optional;
import java.util.regex.Pattern;

public final class ServerDataEnvelope {
    public static final int EXPECTED_VERSION = 1;
    public static final int MAX_PAYLOAD_BYTES = 8192;
    private static final Pattern INTEGER_TOKEN_PATTERN = Pattern.compile("-?(0|[1-9]\\d*)");

    private final int version;
    private final String type;
    private final JsonObject payload;
    private final String rawJson;
    private final int payloadSizeBytes;

    private ServerDataEnvelope(int version, String type, JsonObject payload, String rawJson, int payloadSizeBytes) {
        this.version = version;
        this.type = type;
        this.payload = payload;
        this.rawJson = rawJson;
        this.payloadSizeBytes = payloadSizeBytes;
    }

    public static String decodeUtf8(byte[] bytes) {
        return new String(bytes, StandardCharsets.UTF_8);
    }

    public static ServerPayloadParseResult parse(String jsonPayload, int payloadSizeBytes) {
        if (jsonPayload == null) {
            return ServerPayloadParseResult.error("Malformed JSON: payload was null");
        }
        if (payloadSizeBytes < 0) {
            return ServerPayloadParseResult.error("Malformed JSON: payload byte size cannot be negative");
        }
        if (payloadSizeBytes > MAX_PAYLOAD_BYTES) {
            return ServerPayloadParseResult.error(
                "Payload exceeds max size of " + MAX_PAYLOAD_BYTES + " bytes: " + payloadSizeBytes
            );
        }

        try {
            JsonElement rootElement = JsonParser.parseString(jsonPayload);
            if (!rootElement.isJsonObject()) {
                return ServerPayloadParseResult.error("Malformed JSON: expected top-level object");
            }

            JsonObject root = rootElement.getAsJsonObject();
            Integer version = readRequiredInteger(root, "v");
            if (version == null) {
                return ServerPayloadParseResult.error("Missing version 'v' field");
            }
            if (version != EXPECTED_VERSION) {
                return ServerPayloadParseResult.error("Unsupported version: " + version);
            }

            String type = readRequiredString(root, "type");
            if (type == null || type.isBlank()) {
                return ServerPayloadParseResult.error("Missing required field 'type'");
            }
            if (requiresLegacyMessage(type) && readOptionalString(root, "message").isEmpty()) {
                return ServerPayloadParseResult.error(
                    "Missing required field 'message' for legacy payload type '" + type + "'"
                );
            }

            return ServerPayloadParseResult.success(new ServerDataEnvelope(
                version,
                type,
                root.deepCopy(),
                jsonPayload,
                payloadSizeBytes
            ));
        } catch (RuntimeException exception) {
            return ServerPayloadParseResult.error("Malformed JSON: " + exception.getMessage());
        }
    }

    private static boolean requiresLegacyMessage(String type) {
        return "welcome".equals(type) || "heartbeat".equals(type);
    }

    private static Integer readRequiredInteger(JsonObject root, String fieldName) {
        JsonElement element = root.get(fieldName);
        if (element == null || element.isJsonNull()) {
            return null;
        }

        JsonPrimitive primitive = requirePrimitive(fieldName, element);
        if (!primitive.isNumber()) {
            throw new IllegalStateException("field '" + fieldName + "' must be a number");
        }

        String rawValue = primitive.getAsString();
        if (!INTEGER_TOKEN_PATTERN.matcher(rawValue).matches()) {
            throw new IllegalStateException("field '" + fieldName + "' must be an integer");
        }

        return Integer.parseInt(rawValue);
    }

    private static String readRequiredString(JsonObject root, String fieldName) {
        return readOptionalString(root, fieldName).orElse(null);
    }

    private static Optional<String> readOptionalString(JsonObject root, String fieldName) {
        JsonElement element = root.get(fieldName);
        if (element == null || element.isJsonNull()) {
            return Optional.empty();
        }

        JsonPrimitive primitive = requirePrimitive(fieldName, element);
        if (!primitive.isString()) {
            throw new IllegalStateException("field '" + fieldName + "' must be a string");
        }
        return Optional.of(primitive.getAsString());
    }

    private static JsonPrimitive requirePrimitive(String fieldName, JsonElement element) {
        if (!element.isJsonPrimitive()) {
            throw new IllegalStateException("field '" + fieldName + "' must be a primitive value");
        }
        return element.getAsJsonPrimitive();
    }

    public int version() {
        return version;
    }

    public String type() {
        return type;
    }

    public JsonObject payload() {
        return payload.deepCopy();
    }

    public String rawJson() {
        return rawJson;
    }

    public int payloadSizeBytes() {
        return payloadSizeBytes;
    }

    public Optional<String> message() {
        return readOptionalString(payload, "message");
    }
}
