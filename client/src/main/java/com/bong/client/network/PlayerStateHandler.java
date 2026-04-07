package com.bong.client.network;

import com.bong.client.state.PlayerStateViewModel;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.ArrayList;
import java.util.List;

public final class PlayerStateHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        List<String> invalidFields = new ArrayList<>();

        String realm = readRequiredString(payload, "realm");
        if (realm == null) {
            invalidFields.add("realm");
        }

        Double spiritQiCurrent = readRequiredDouble(payload, "spirit_qi_current", "spirit_qi");
        if (spiritQiCurrent == null) {
            invalidFields.add("spirit_qi_current");
        }

        Double karma = readRequiredDouble(payload, "karma");
        if (karma == null) {
            invalidFields.add("karma");
        }

        Double compositePower = readRequiredDouble(payload, "composite_power");
        if (compositePower == null) {
            invalidFields.add("composite_power");
        }

        PlayerStateViewModel.PowerBreakdown breakdown = readRequiredBreakdown(payload, "breakdown");
        if (breakdown == null) {
            invalidFields.add("breakdown");
        }

        String zoneId = readRequiredString(payload, "zone");
        if (zoneId == null) {
            invalidFields.add("zone");
        }

        Double zoneSpiritQi = readRequiredDouble(payload, "zone_spirit_qi");
        if (zoneSpiritQi == null) {
            invalidFields.add("zone_spirit_qi");
        }

        if (!invalidFields.isEmpty()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring player_state payload because required fields are missing or invalid: " + String.join(", ", invalidFields)
            );
        }

        PlayerStateViewModel playerStateViewModel = PlayerStateViewModel.create(
            realm,
            spiritQiCurrent,
            readOptionalDouble(payload, "spirit_qi_max", Double.NaN),
            karma,
            compositePower,
            breakdown,
            zoneId,
            readOptionalString(payload, "zone_label"),
            zoneSpiritQi
        );

        if (playerStateViewModel.isEmpty()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring player_state payload because it normalized to an empty view model"
            );
        }

        return ServerDataDispatch.handledWithPlayerState(
            envelope.type(),
            playerStateViewModel,
            "Routed player_state payload into player state view model"
        );
    }

    private static PlayerStateViewModel.PowerBreakdown readRequiredBreakdown(JsonObject payload, String fieldName) {
        JsonElement element = payload.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonObject()) {
            return null;
        }

        JsonObject breakdownObject = element.getAsJsonObject();
        Double combat = readRequiredDouble(breakdownObject, "combat");
        Double wealth = readRequiredDouble(breakdownObject, "wealth");
        Double social = readRequiredDouble(breakdownObject, "social");
        Double territory = readRequiredDouble(breakdownObject, "territory");
        if (combat == null || wealth == null || social == null || territory == null) {
            return null;
        }

        return PlayerStateViewModel.PowerBreakdown.create(combat, wealth, social, territory);
    }

    private static String readRequiredString(JsonObject payload, String fieldName) {
        String value = readOptionalString(payload, fieldName);
        if (value == null || value.isBlank()) {
            return null;
        }

        return value;
    }

    private static String readOptionalString(JsonObject payload, String fieldName) {
        JsonElement element = payload.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }

        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isString()) {
            return null;
        }

        return primitive.getAsString().trim();
    }

    private static Double readRequiredDouble(JsonObject payload, String primaryFieldName, String... alternateFieldNames) {
        Double value = readOptionalDouble(payload, primaryFieldName, null);
        if (value != null) {
            return value;
        }

        for (String alternateFieldName : alternateFieldNames) {
            value = readOptionalDouble(payload, alternateFieldName, null);
            if (value != null) {
                return value;
            }
        }

        return null;
    }

    private static Double readOptionalDouble(JsonObject payload, String fieldName, Double defaultValue) {
        JsonElement element = payload.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return defaultValue;
        }

        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isNumber()) {
            return defaultValue;
        }

        double value = primitive.getAsDouble();
        return Double.isFinite(value) ? value : defaultValue;
    }
}
