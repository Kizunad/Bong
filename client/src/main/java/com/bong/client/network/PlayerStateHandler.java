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

        Double zoneSpiritQi = readOptionalDouble(payload, "zone_spirit_qi", Double.NaN);

        if (!invalidFields.isEmpty()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring player_state payload because required fields are missing or invalid: " + String.join(", ", invalidFields)
            );
        }

        PlayerStateViewModel playerStateViewModel = PlayerStateViewModel.create(
            realm,
            readOptionalString(payload, "player"),
            spiritQiCurrent,
            readOptionalDouble(payload, "spirit_qi_max", Double.NaN),
            karma,
            compositePower,
            breakdown,
            readOptionalSocialSnapshot(payload),
            zoneId,
            readOptionalString(payload, "zone_label"),
            zoneSpiritQi,
            readOptionalDouble(payload, "local_neg_pressure", 0.0)
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

    private static PlayerStateViewModel.SocialSnapshot readOptionalSocialSnapshot(JsonObject payload) {
        JsonElement element = payload.get("social");
        if (element == null || element.isJsonNull() || !element.isJsonObject()) {
            return PlayerStateViewModel.SocialSnapshot.empty();
        }

        JsonObject social = element.getAsJsonObject();
        JsonObject renown = readObject(social, "renown");
        int fame = readOptionalInt(renown, "fame", 0);
        int notoriety = readOptionalInt(renown, "notoriety", 0);
        List<String> topTags = readTopTags(renown);

        JsonObject faction = readObject(social, "faction_membership");
        return PlayerStateViewModel.SocialSnapshot.create(
            fame,
            notoriety,
            topTags,
            readOptionalString(faction, "faction"),
            readOptionalInt(faction, "rank", 0),
            readOptionalInt(faction, "loyalty", 0),
            readOptionalInt(faction, "betrayal_count", 0)
        );
    }

    private static List<String> readTopTags(JsonObject renown) {
        JsonElement element = renown == null ? null : renown.get("top_tags");
        if (element == null || element.isJsonNull() || !element.isJsonArray()) {
            return List.of();
        }
        List<String> tags = new ArrayList<>();
        for (JsonElement tagElement : element.getAsJsonArray()) {
            if (!tagElement.isJsonObject()) continue;
            String tag = readOptionalString(tagElement.getAsJsonObject(), "tag");
            if (tag != null && !tag.isBlank()) {
                tags.add(tag);
            }
        }
        return List.copyOf(tags);
    }

    private static JsonObject readObject(JsonObject payload, String fieldName) {
        JsonElement element = payload == null ? null : payload.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonObject()) {
            return null;
        }
        return element.getAsJsonObject();
    }

    private static String readRequiredString(JsonObject payload, String fieldName) {
        String value = readOptionalString(payload, fieldName);
        if (value == null || value.isBlank()) {
            return null;
        }

        return value;
    }

    private static String readOptionalString(JsonObject payload, String fieldName) {
        if (payload == null) {
            return null;
        }
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
        if (payload == null) {
            return defaultValue;
        }
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

    private static int readOptionalInt(JsonObject payload, String fieldName, int defaultValue) {
        Double value = readOptionalDouble(payload, fieldName, null);
        return value == null ? defaultValue : value.intValue();
    }
}
