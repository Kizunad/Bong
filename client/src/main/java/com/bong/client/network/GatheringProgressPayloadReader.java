package com.bong.client.network;

import com.bong.client.gathering.GatheringSessionStore;
import com.bong.client.gathering.GatheringSessionViewModel;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

final class GatheringProgressPayloadReader {
    private GatheringProgressPayloadReader() {
    }

    static ServerDataDispatch apply(
        ServerDataEnvelope envelope,
        String targetType,
        String defaultTargetName,
        String... targetNameFields
    ) {
        JsonObject payload = envelope.payload();
        String sessionId = readOptionalString(payload, "session_id");
        if (sessionId == null || sessionId.isBlank()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring " + envelope.type() + " payload: required field 'session_id' is missing or invalid"
            );
        }

        Double progress = readOptionalDouble(payload, "progress");
        if (progress == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring " + envelope.type() + " payload: required field 'progress' is missing or invalid"
            );
        }

        boolean interrupted = readOptionalBoolean(payload, "interrupted") == Boolean.TRUE;
        boolean completed = readOptionalBoolean(payload, "completed") == Boolean.TRUE || progress >= 1.0;
        GatheringSessionViewModel current = GatheringSessionStore.snapshot();
        if (interrupted || completed) {
            if (sessionId.trim().equals(current.sessionId())) {
                // 两类进度消息共用 HUD 终态；不能先清空再让 gathering_session 重播退场。
                GatheringSessionStore.replace(GatheringSessionViewModel.create(
                    current.sessionId(), completed ? current.totalTicks() : current.progressTicks(), current.totalTicks(),
                    current.targetName(), current.targetType(), current.qualityHint(), current.toolUsed(),
                    interrupted, completed, System.currentTimeMillis()));
            }
            return ServerDataDispatch.handled(
                envelope.type(),
                "Finished gathering progress '" + sessionId.trim() + "' from " + envelope.type()
            );
        }

        GatheringSessionViewModel model = GatheringSessionViewModel.createFromProgressRatio(
            sessionId,
            progress,
            firstNonBlank(payload, defaultTargetName, targetNameFields),
            targetType,
            false,
            false,
            System.currentTimeMillis()
        );
        GatheringSessionStore.replace(model);
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied " + envelope.type() + " '" + model.sessionId() + "' to GatheringSessionStore"
        );
    }

    private static String firstNonBlank(JsonObject object, String fallback, String... fieldNames) {
        for (String fieldName : fieldNames) {
            String value = readOptionalString(object, fieldName);
            if (value != null && !value.isBlank()) {
                return value;
            }
        }
        return fallback;
    }

    private static String readOptionalString(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isString()) {
            return null;
        }
        return primitive.getAsString();
    }

    private static Double readOptionalDouble(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isNumber()) {
            return null;
        }
        try {
            double value = primitive.getAsDouble();
            return Double.isFinite(value) ? value : null;
        } catch (NumberFormatException error) {
            return null;
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
