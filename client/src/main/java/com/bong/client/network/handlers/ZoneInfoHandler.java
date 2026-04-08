package com.bong.client.network.handlers;

import com.bong.client.BongClient;
import com.bong.client.ZoneHudState;
import com.bong.client.network.PayloadHandler;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParseException;
import com.google.gson.JsonParser;
import net.minecraft.client.MinecraftClient;

import java.util.ArrayList;
import java.util.List;

public class ZoneInfoHandler implements PayloadHandler {
    @Override
    public void handle(MinecraftClient client, String type, String jsonPayload) {
        handlePayload(jsonPayload);
    }

    void handlePayload(String jsonPayload) {
        ParseResult result = parse(jsonPayload);
        if (!result.success()) {
            BongClient.LOGGER.warn("Ignoring malformed zone_info payload: {}", result.errorMessage());
            return;
        }

        ZoneHudState.update(
            result.zone(),
            result.spiritQi(),
            result.dangerLevel(),
            result.activeEvents()
        );
    }

    private ParseResult parse(String jsonPayload) {
        try {
            JsonElement rootElement = JsonParser.parseString(jsonPayload);
            if (!rootElement.isJsonObject()) {
                return ParseResult.error("Zone payload root must be an object");
            }

            JsonObject rootObject = rootElement.getAsJsonObject();
            String zone = getRequiredString(rootObject, "zone");
            if (zone == null || zone.isBlank()) {
                return ParseResult.error("Zone payload missing required string field 'zone'");
            }

            Double spiritQi = getRequiredNumber(rootObject, "spirit_qi");
            if (spiritQi == null || !Double.isFinite(spiritQi) || spiritQi < 0.0 || spiritQi > 1.0) {
                return ParseResult.error("Zone payload has invalid 'spirit_qi'");
            }

            Integer dangerLevel = getRequiredInteger(rootObject, "danger_level");
            if (dangerLevel == null || dangerLevel < 0 || dangerLevel > 5) {
                return ParseResult.error("Zone payload has invalid 'danger_level'");
            }

            List<String> activeEvents = parseOptionalStringArray(rootObject, "active_events");
            if (activeEvents == null) {
                return ParseResult.error("Zone payload has invalid 'active_events'");
            }

            return ParseResult.success(zone, spiritQi, dangerLevel, activeEvents);
        } catch (JsonParseException | IllegalStateException exception) {
            return ParseResult.error("Malformed zone payload");
        }
    }

    private static String getRequiredString(JsonObject object, String key) {
        JsonElement value = object.get(key);
        if (value == null || !value.isJsonPrimitive() || !value.getAsJsonPrimitive().isString()) {
            return null;
        }

        return value.getAsString();
    }

    private static Double getRequiredNumber(JsonObject object, String key) {
        JsonElement value = object.get(key);
        if (value == null || !value.isJsonPrimitive() || !value.getAsJsonPrimitive().isNumber()) {
            return null;
        }

        return value.getAsDouble();
    }

    private static Integer getRequiredInteger(JsonObject object, String key) {
        Double number = getRequiredNumber(object, key);
        if (number == null || !Double.isFinite(number)) {
            return null;
        }

        int asInt = number.intValue();
        if (Math.abs(number - asInt) > 1e-9) {
            return null;
        }

        return asInt;
    }

    private static List<String> parseOptionalStringArray(JsonObject object, String key) {
        JsonElement value = object.get(key);
        if (value == null || value.isJsonNull()) {
            return List.of();
        }
        if (!value.isJsonArray()) {
            return null;
        }

        JsonArray array = value.getAsJsonArray();
        List<String> items = new ArrayList<>(array.size());
        for (JsonElement element : array) {
            if (!element.isJsonPrimitive() || !element.getAsJsonPrimitive().isString()) {
                return null;
            }

            String item = element.getAsString();
            if (!item.isBlank()) {
                items.add(item);
            }
        }

        return items;
    }

    private record ParseResult(
        boolean success,
        String zone,
        double spiritQi,
        int dangerLevel,
        List<String> activeEvents,
        String errorMessage
    ) {
        private static ParseResult success(String zone, double spiritQi, int dangerLevel, List<String> activeEvents) {
            return new ParseResult(true, zone, spiritQi, dangerLevel, List.copyOf(activeEvents), null);
        }

        private static ParseResult error(String message) {
            return new ParseResult(false, null, 0.0, 0, List.of(), message);
        }
    }
}
