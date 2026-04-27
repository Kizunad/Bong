package com.bong.client;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParseException;
import com.google.gson.JsonParser;
import com.google.gson.JsonPrimitive;
import com.google.gson.stream.JsonReader;
import com.google.gson.stream.JsonToken;

import java.io.IOException;
import java.io.StringReader;
import java.util.ArrayList;
import java.util.List;

public final class BongServerPayloadParser {
    private BongServerPayloadParser() {
    }

    public static BongNetworkHandler.ParseResult parse(String jsonPayload) {
        try {
            JsonElement root = parseStrictJson(jsonPayload);
            if (!root.isJsonObject()) {
                return BongNetworkHandler.ParseResult.error("Malformed JSON: expected top-level object");
            }

            JsonObject payloadObject = root.getAsJsonObject();
            int version = readRequiredInt(payloadObject, "v");
            if (version != BongNetworkHandler.EXPECTED_VERSION) {
                return BongNetworkHandler.ParseResult.error("Unsupported version: " + version);
            }

            String type = readRequiredString(payloadObject, "type");
            BongServerPayload.BongServerPayloadKind kind = BongServerPayload.BongServerPayloadKind.fromWireName(type);
            if (kind == null) {
                return BongNetworkHandler.ParseResult.error("Unknown payload type: " + type);
            }

            BongServerPayload payload = switch (kind) {
                case WELCOME -> new BongServerPayload.WelcomePayload(version, readRequiredString(payloadObject, "message"));
                case HEARTBEAT -> new BongServerPayload.HeartbeatPayload(version, readRequiredString(payloadObject, "message"));
                case NARRATION -> parseNarrationPayload(payloadObject, version);
                case ZONE_INFO -> parseZoneInfoPayload(payloadObject, version);
                case EVENT_ALERT -> parseEventAlertPayload(payloadObject, version);
                case PLAYER_STATE -> parsePlayerStatePayload(payloadObject, version);
            };

            return BongNetworkHandler.ParseResult.success(payload);
        } catch (PayloadParseException exception) {
            return BongNetworkHandler.ParseResult.error(exception.getMessage());
        } catch (IOException exception) {
            return BongNetworkHandler.ParseResult.error("Malformed JSON: " + exception.getMessage());
        } catch (JsonParseException | IllegalStateException exception) {
            return BongNetworkHandler.ParseResult.error("Malformed JSON: " + exception.getMessage());
        }
    }

    private static JsonElement parseStrictJson(String jsonPayload) throws IOException {
        JsonReader reader = new JsonReader(new StringReader(jsonPayload));
        reader.setLenient(false);

        JsonElement root = JsonParser.parseReader(reader);
        JsonToken trailingToken = reader.peek();
        if (trailingToken != JsonToken.END_DOCUMENT) {
            throw new JsonParseException("unexpected trailing token " + trailingToken);
        }

        return root;
    }

    private static BongServerPayload.NarrationPayload parseNarrationPayload(JsonObject payloadObject, int version)
            throws PayloadParseException {
        JsonArray narrationArray = readRequiredArray(payloadObject, "narrations");
        if (narrationArray.size() == 0) {
            throw new PayloadParseException("Invalid field 'narrations': expected at least one entry");
        }

        List<BongServerPayload.Narration> narrations = new ArrayList<>();
        for (int index = 0; index < narrationArray.size(); index++) {
            JsonObject narrationObject = requireObject(narrationArray.get(index), "narrations[" + index + "]");
            narrations.add(new BongServerPayload.Narration(
                    readRequiredString(narrationObject, "scope"),
                    readRequiredString(narrationObject, "text"),
                    readRequiredString(narrationObject, "style"),
                    readOptionalString(narrationObject, "kind")
            ));
        }

        return new BongServerPayload.NarrationPayload(version, narrations);
    }

    private static BongServerPayload.ZoneInfoPayload parseZoneInfoPayload(JsonObject payloadObject, int version)
            throws PayloadParseException {
        JsonObject zoneInfoObject = readRequiredObject(payloadObject, "zone_info");
        BongServerPayload.ZoneInfo zoneInfo = new BongServerPayload.ZoneInfo(
                readRequiredString(zoneInfoObject, "zone"),
                readRequiredDouble(zoneInfoObject, "spirit_qi"),
                readRequiredInt(zoneInfoObject, "danger_level"),
                readOptionalStringArray(zoneInfoObject, "active_events")
        );

        return new BongServerPayload.ZoneInfoPayload(version, zoneInfo);
    }

    private static BongServerPayload.EventAlertPayload parseEventAlertPayload(JsonObject payloadObject, int version)
            throws PayloadParseException {
        JsonObject eventAlertObject = readRequiredObject(payloadObject, "event_alert");
        BongServerPayload.EventAlert eventAlert = new BongServerPayload.EventAlert(
                readRequiredString(eventAlertObject, "kind"),
                readRequiredString(eventAlertObject, "title"),
                readRequiredString(eventAlertObject, "detail"),
                readRequiredString(eventAlertObject, "severity"),
                readOptionalString(eventAlertObject, "zone")
        );

        return new BongServerPayload.EventAlertPayload(version, eventAlert);
    }

    private static BongServerPayload.PlayerStatePayload parsePlayerStatePayload(JsonObject payloadObject, int version)
            throws PayloadParseException {
        JsonObject playerStateObject = readRequiredObject(payloadObject, "player_state");
        BongServerPayload.PlayerState playerState = new BongServerPayload.PlayerState(
                readRequiredString(playerStateObject, "realm"),
                readRequiredDouble(playerStateObject, "spirit_qi"),
                readRequiredDouble(playerStateObject, "spirit_qi_max"),
                readRequiredDouble(playerStateObject, "karma"),
                readRequiredDouble(playerStateObject, "composite_power"),
                readRequiredString(playerStateObject, "zone")
        );

        return new BongServerPayload.PlayerStatePayload(version, playerState);
    }

    private static JsonObject readRequiredObject(JsonObject parent, String fieldName) throws PayloadParseException {
        JsonElement element = parent.get(fieldName);
        if (element == null || element.isJsonNull()) {
            throw new PayloadParseException("Missing required field '" + fieldName + "'");
        }

        return requireObject(element, fieldName);
    }

    private static JsonObject requireObject(JsonElement element, String fieldName) throws PayloadParseException {
        if (!element.isJsonObject()) {
            throw new PayloadParseException("Invalid field '" + fieldName + "': expected object");
        }

        return element.getAsJsonObject();
    }

    private static JsonArray readRequiredArray(JsonObject parent, String fieldName) throws PayloadParseException {
        JsonElement element = parent.get(fieldName);
        if (element == null || element.isJsonNull()) {
            throw new PayloadParseException("Missing required field '" + fieldName + "'");
        }

        if (!element.isJsonArray()) {
            throw new PayloadParseException("Invalid field '" + fieldName + "': expected array");
        }

        return element.getAsJsonArray();
    }

    private static String readRequiredString(JsonObject parent, String fieldName) throws PayloadParseException {
        JsonElement element = parent.get(fieldName);
        if (element == null || element.isJsonNull()) {
            throw new PayloadParseException("Missing required field '" + fieldName + "'");
        }

        JsonPrimitive primitive = requirePrimitive(element, fieldName);
        if (!primitive.isString()) {
            throw new PayloadParseException("Invalid field '" + fieldName + "': expected string");
        }

        String value = primitive.getAsString();
        if (value.isEmpty()) {
            throw new PayloadParseException("Invalid field '" + fieldName + "': expected non-empty string");
        }

        return value;
    }

    private static String readOptionalString(JsonObject parent, String fieldName) throws PayloadParseException {
        if (!parent.has(fieldName)) {
            return null;
        }

        JsonElement element = parent.get(fieldName);
        if (element == null || element.isJsonNull()) {
            throw new PayloadParseException("Invalid field '" + fieldName + "': expected string");
        }

        JsonPrimitive primitive = requirePrimitive(element, fieldName);
        if (!primitive.isString()) {
            throw new PayloadParseException("Invalid field '" + fieldName + "': expected string");
        }

        String value = primitive.getAsString();
        if (value.isEmpty()) {
            return null;
        }

        return value;
    }

    private static int readRequiredInt(JsonObject parent, String fieldName) throws PayloadParseException {
        JsonPrimitive primitive = requirePrimitive(parent.get(fieldName), fieldName);
        if (!primitive.isNumber()) {
            throw new PayloadParseException("Invalid field '" + fieldName + "': expected integer");
        }

        try {
            return Integer.parseInt(primitive.getAsString());
        } catch (NumberFormatException exception) {
            throw new PayloadParseException("Invalid field '" + fieldName + "': expected integer");
        }
    }

    private static double readRequiredDouble(JsonObject parent, String fieldName) throws PayloadParseException {
        JsonPrimitive primitive = requirePrimitive(parent.get(fieldName), fieldName);
        if (!primitive.isNumber()) {
            throw new PayloadParseException("Invalid field '" + fieldName + "': expected number");
        }

        double value;
        try {
            value = primitive.getAsDouble();
        } catch (NumberFormatException exception) {
            throw new PayloadParseException("Invalid field '" + fieldName + "': expected number");
        }

        if (!Double.isFinite(value)) {
            throw new PayloadParseException("Invalid field '" + fieldName + "': expected finite number");
        }

        return value;
    }

    private static List<String> readOptionalStringArray(JsonObject parent, String fieldName) throws PayloadParseException {
        if (!parent.has(fieldName)) {
            return List.of();
        }

        JsonElement element = parent.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonArray()) {
            throw new PayloadParseException("Invalid field '" + fieldName + "': expected array");
        }

        JsonArray array = element.getAsJsonArray();
        List<String> values = new ArrayList<>();
        for (int index = 0; index < array.size(); index++) {
            JsonPrimitive primitive = requirePrimitive(array.get(index), fieldName + "[" + index + "]");
            if (!primitive.isString()) {
                throw new PayloadParseException("Invalid field '" + fieldName + "[" + index + "]': expected string");
            }

            String value = primitive.getAsString();
            if (value.isEmpty()) {
                throw new PayloadParseException("Invalid field '" + fieldName + "[" + index + "]': expected non-empty string");
            }

            values.add(value);
        }

        return values;
    }

    private static JsonPrimitive requirePrimitive(JsonElement element, String fieldName) throws PayloadParseException {
        if (element == null || element.isJsonNull()) {
            throw new PayloadParseException("Missing required field '" + fieldName + "'");
        }

        if (!element.isJsonPrimitive()) {
            throw new PayloadParseException("Invalid field '" + fieldName + "': expected primitive value");
        }

        return element.getAsJsonPrimitive();
    }

    private static final class PayloadParseException extends Exception {
        private PayloadParseException(String message) {
            super(message);
        }
    }
}
