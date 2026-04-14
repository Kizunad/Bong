package com.bong.client.network;

import com.bong.client.inventory.state.InventoryStateStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.Set;
import java.util.regex.Pattern;

public final class InventoryEventHandler implements ServerDataHandler {
    private static final long JS_SAFE_INTEGER_MAX = 9_007_199_254_740_991L;
    private static final Pattern INTEGER_TOKEN_PATTERN = Pattern.compile("-?(0|[1-9]\\d*)");
    private static final Set<String> SUPPORTED_KINDS = Set.of(
        "moved",
        "stack_changed",
        "durability_changed"
    );

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        if (!InventoryStateStore.isAuthoritativeLoaded()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring inventory_event payload because authoritative inventory snapshot is not loaded yet"
            );
        }

        JsonObject payload = envelope.payload();
        Long revision = readRequiredLong(payload, "revision");
        String kind = readRequiredString(payload, "kind");
        if (revision == null || kind == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring inventory_event payload because required fields 'kind' or 'revision' are missing or invalid"
            );
        }

        long currentRevision = InventoryStateStore.revision();
        if (revision < currentRevision) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring inventory_event payload because revision " + revision + " is stale (store revision " + currentRevision + ")"
            );
        }

        if (!SUPPORTED_KINDS.contains(kind)) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring inventory_event payload because kind '" + kind + "' is unsupported"
            );
        }

        return ServerDataDispatch.noOp(
            envelope.type(),
            "Ignoring inventory_event payload kind '" + kind + "' in Task 8 safety mode; authoritative snapshot remains source of truth"
        );
    }

    private static String readRequiredString(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }

        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isString()) {
            return null;
        }

        String value = primitive.getAsString().trim();
        return value.isEmpty() ? null : value;
    }

    private static Long readRequiredLong(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }

        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isNumber()) {
            return null;
        }

        String token = primitive.getAsString();
        if (!INTEGER_TOKEN_PATTERN.matcher(token).matches()) {
            return null;
        }

        long value;
        try {
            value = Long.parseLong(token);
        } catch (NumberFormatException exception) {
            return null;
        }

        if (value < 0 || value > JS_SAFE_INTEGER_MAX) {
            return null;
        }

        return value;
    }
}
