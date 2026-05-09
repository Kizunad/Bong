package com.bong.client.network;

import com.bong.client.identity.IdentityPanelEntry;
import com.bong.client.identity.IdentityPanelState;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.ArrayList;
import java.util.List;
import java.util.regex.Pattern;

/** plan-identity-v1 P5：解析 server_data identity_panel_state 并交给 client store。 */
public final class IdentityPanelStateHandler implements ServerDataHandler {
    private static final Pattern INTEGER_TOKEN_PATTERN = Pattern.compile("-?(0|[1-9]\\d*)");

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        if (!"identity_panel_state".equals(envelope.type())) {
            return ServerDataDispatch.noOp(envelope.type(), "Ignoring unsupported identity panel payload type");
        }

        JsonObject payload = envelope.payload();
        Integer activeIdentityId = readInt(payload, "active_identity_id");
        Long lastSwitchTick = readLong(payload, "last_switch_tick");
        Long cooldownRemainingTicks = readLong(payload, "cooldown_remaining_ticks");
        JsonArray identitiesArray = readArray(payload, "identities");
        if (activeIdentityId == null || lastSwitchTick == null || cooldownRemainingTicks == null || identitiesArray == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring identity_panel_state: missing active_identity_id/last_switch_tick/cooldown_remaining_ticks/identities"
            );
        }

        List<IdentityPanelEntry> identities = new ArrayList<>();
        for (JsonElement identityElement : identitiesArray) {
            IdentityPanelEntry entry = parseEntry(identityElement);
            if (entry == null) {
                return ServerDataDispatch.noOp(envelope.type(), "Ignoring identity_panel_state: malformed identity entry");
            }
            identities.add(entry);
        }

        IdentityPanelState state = new IdentityPanelState(
            activeIdentityId,
            lastSwitchTick,
            cooldownRemainingTicks,
            identities
        );
        return ServerDataDispatch.handledWithIdentityPanelState(
            envelope.type(),
            state,
            "Applied identity_panel_state (" + identities.size() + " identities)"
        );
    }

    private static IdentityPanelEntry parseEntry(JsonElement element) {
        if (element == null || !element.isJsonObject()) {
            return null;
        }
        JsonObject object = element.getAsJsonObject();
        Integer identityId = readInt(object, "identity_id");
        String displayName = readString(object, "display_name");
        Integer reputationScore = readInt(object, "reputation_score");
        Boolean frozen = readBoolean(object, "frozen");
        JsonArray tagArray = readArray(object, "revealed_tag_kinds");
        if (identityId == null || displayName == null || reputationScore == null || frozen == null || tagArray == null) {
            return null;
        }

        List<String> revealedTagKinds = new ArrayList<>();
        for (JsonElement tagElement : tagArray) {
            String tag = readString(tagElement);
            if (tag == null) {
                return null;
            }
            revealedTagKinds.add(tag);
        }
        return new IdentityPanelEntry(identityId, displayName, reputationScore, frozen, revealedTagKinds);
    }

    private static Integer readInt(JsonObject object, String key) {
        JsonPrimitive primitive = readPrimitive(object, key);
        if (primitive == null || !primitive.isNumber()) {
            return null;
        }
        if (!INTEGER_TOKEN_PATTERN.matcher(primitive.getAsString()).matches()) {
            return null;
        }
        try {
            return primitive.getAsInt();
        } catch (NumberFormatException | ClassCastException ex) {
            return null;
        }
    }

    private static Long readLong(JsonObject object, String key) {
        JsonPrimitive primitive = readPrimitive(object, key);
        if (primitive == null || !primitive.isNumber()) {
            return null;
        }
        if (!INTEGER_TOKEN_PATTERN.matcher(primitive.getAsString()).matches()) {
            return null;
        }
        try {
            return primitive.getAsLong();
        } catch (NumberFormatException | ClassCastException ex) {
            return null;
        }
    }

    private static String readString(JsonObject object, String key) {
        return readString(object == null ? null : object.get(key));
    }

    private static String readString(JsonElement element) {
        if (element == null || !element.isJsonPrimitive()) {
            return null;
        }
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        return primitive.isString() ? primitive.getAsString() : null;
    }

    private static Boolean readBoolean(JsonObject object, String key) {
        JsonPrimitive primitive = readPrimitive(object, key);
        if (primitive == null || !primitive.isBoolean()) {
            return null;
        }
        return primitive.getAsBoolean();
    }

    private static JsonArray readArray(JsonObject object, String key) {
        JsonElement element = object == null ? null : object.get(key);
        return element != null && element.isJsonArray() ? element.getAsJsonArray() : null;
    }

    private static JsonPrimitive readPrimitive(JsonObject object, String key) {
        JsonElement element = object == null ? null : object.get(key);
        return element != null && element.isJsonPrimitive() ? element.getAsJsonPrimitive() : null;
    }
}
