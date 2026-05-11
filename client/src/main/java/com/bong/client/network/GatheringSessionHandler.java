package com.bong.client.network;

import com.bong.client.gathering.GatheringSessionStore;
import com.bong.client.gathering.GatheringSessionViewModel;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

public final class GatheringSessionHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        String sessionId = readOptionalString(payload, "session_id");
        if (sessionId == null || sessionId.isBlank()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring gathering_session payload: required field 'session_id' is missing or invalid"
            );
        }

        GatheringSessionViewModel model = GatheringSessionViewModel.create(
            sessionId,
            readOptionalLong(payload, "progress_ticks", 0L),
            readOptionalLong(payload, "total_ticks", 0L),
            readOptionalString(payload, "target_name"),
            readOptionalString(payload, "target_type"),
            readOptionalString(payload, "quality_hint"),
            readOptionalString(payload, "tool_used"),
            readOptionalBoolean(payload, "interrupted") == Boolean.TRUE,
            readOptionalBoolean(payload, "completed") == Boolean.TRUE,
            System.currentTimeMillis()
        );
        GatheringSessionStore.replace(model);
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied gathering_session '" + model.sessionId() + "' to GatheringSessionStore"
        );
    }

    private static String readOptionalString(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isString()) {
            return null;
        }
        return primitive.getAsString();
    }

    private static long readOptionalLong(JsonObject object, String fieldName, long fallback) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isNumber()) {
            return fallback;
        }
        try {
            return Math.max(0L, primitive.getAsLong());
        } catch (NumberFormatException error) {
            return fallback;
        }
    }

    private static Boolean readOptionalBoolean(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isBoolean()) {
            return null;
        }
        return primitive.getAsBoolean();
    }

    private static JsonPrimitive readPrimitive(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }
        return element.getAsJsonPrimitive();
    }
}
