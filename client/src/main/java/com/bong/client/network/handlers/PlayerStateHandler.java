package com.bong.client.network.handlers;

import com.bong.client.BongClient;
import com.bong.client.PlayerStateCache;
import com.bong.client.network.PayloadHandler;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParseException;
import com.google.gson.JsonParser;
import net.minecraft.client.MinecraftClient;

public class PlayerStateHandler implements PayloadHandler {
    @Override
    public void handle(MinecraftClient client, String type, String jsonPayload) {
        handlePayload(jsonPayload);
    }

    void handlePayload(String jsonPayload) {
        ParseResult result = parse(jsonPayload);
        if (!result.success()) {
            BongClient.LOGGER.warn("Ignoring malformed player_state payload: {}", result.errorMessage());
            return;
        }

        PlayerStateCache.update(result.snapshot());
    }

    private ParseResult parse(String jsonPayload) {
        try {
            JsonElement rootElement = JsonParser.parseString(jsonPayload);
            if (!rootElement.isJsonObject()) {
                return ParseResult.error("Player state root must be an object");
            }

            JsonObject rootObject = rootElement.getAsJsonObject();
            String realm = getRequiredString(rootObject, "realm");
            if (realm == null || realm.isBlank()) {
                return ParseResult.error("Player state missing required string field 'realm'");
            }

            Double spiritQi = getRequiredNumber(rootObject, "spirit_qi");
            if (spiritQi == null || !Double.isFinite(spiritQi) || spiritQi < 0.0) {
                return ParseResult.error("Player state has invalid 'spirit_qi'");
            }

            Double karma = getRequiredNumber(rootObject, "karma");
            if (karma == null || !Double.isFinite(karma) || karma < -1.0 || karma > 1.0) {
                return ParseResult.error("Player state has invalid 'karma'");
            }

            Double compositePower = getRequiredNumber(rootObject, "composite_power");
            if (compositePower == null || !Double.isFinite(compositePower) || compositePower < 0.0 || compositePower > 1.0) {
                return ParseResult.error("Player state has invalid 'composite_power'");
            }

            PlayerStateCache.PowerBreakdown breakdown = parseBreakdown(rootObject, "breakdown");
            if (breakdown == null) {
                return ParseResult.error("Player state has invalid 'breakdown'");
            }

            String zone = getRequiredString(rootObject, "zone");
            if (zone == null || zone.isBlank()) {
                return ParseResult.error("Player state missing required string field 'zone'");
            }

            return ParseResult.success(new PlayerStateCache.PlayerStateSnapshot(
                realm,
                spiritQi,
                karma,
                compositePower,
                breakdown,
                zone
            ));
        } catch (JsonParseException | IllegalStateException exception) {
            return ParseResult.error("Malformed player state payload");
        }
    }

    private static PlayerStateCache.PowerBreakdown parseBreakdown(JsonObject object, String key) {
        JsonElement breakdownValue = object.get(key);
        if (breakdownValue == null || !breakdownValue.isJsonObject()) {
            return null;
        }

        JsonObject breakdownObject = breakdownValue.getAsJsonObject();
        Double combat = getRequiredNumber(breakdownObject, "combat");
        Double wealth = getRequiredNumber(breakdownObject, "wealth");
        Double social = getRequiredNumber(breakdownObject, "social");
        Double karma = getRequiredNumber(breakdownObject, "karma");
        Double territory = getRequiredNumber(breakdownObject, "territory");
        if (!isUnitValue(combat) || !isUnitValue(wealth) || !isUnitValue(social) || !isUnitValue(karma) || !isUnitValue(territory)) {
            return null;
        }

        return new PlayerStateCache.PowerBreakdown(combat, wealth, social, karma, territory);
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

    private static boolean isUnitValue(Double value) {
        return value != null && Double.isFinite(value) && value >= 0.0 && value <= 1.0;
    }

    private record ParseResult(boolean success, PlayerStateCache.PlayerStateSnapshot snapshot, String errorMessage) {
        private static ParseResult success(PlayerStateCache.PlayerStateSnapshot snapshot) {
            return new ParseResult(true, snapshot, null);
        }

        private static ParseResult error(String errorMessage) {
            return new ParseResult(false, null, errorMessage);
        }
    }
}
