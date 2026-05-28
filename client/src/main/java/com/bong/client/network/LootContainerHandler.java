package com.bong.client.network;

import com.bong.client.hud.LootContainerStateStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

public final class LootContainerHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        return switch (envelope.type()) {
            case "loot_container_open" -> handleOpen(envelope.type(), payload);
            case "loot_container_update" -> handleUpdate(envelope.type(), payload);
            case "loot_container_close" -> handleClose(envelope.type(), payload);
            default -> ServerDataDispatch.noOp(
                envelope.type(),
                "Unsupported loot container payload type " + envelope.type()
            );
        };
    }

    private static ServerDataDispatch handleOpen(String type, JsonObject payload) {
        Long sessionId = readLong(payload, "session_id");
        if (sessionId == null) {
            return ServerDataDispatch.noOp(type, "Ignoring loot_container_open: missing session_id");
        }

        int rows = readInt(payload, "rows", 3);
        int cols = readInt(payload, "cols", 4);
        long timeoutWallSecs = readLong(payload, "timeout_wall_secs") != null
            ? readLong(payload, "timeout_wall_secs") : 0L;

        String sourceKind = "unknown";
        String grade = "common";
        JsonElement sourceKindEl = payload.get("source_kind");
        if (sourceKindEl != null && sourceKindEl.isJsonObject()) {
            JsonObject sk = sourceKindEl.getAsJsonObject();
            sourceKind = readString(sk, "kind", "unknown");
            grade = readString(sk, "grade", "common");
        } else if (sourceKindEl != null && sourceKindEl.isJsonPrimitive()) {
            sourceKind = sourceKindEl.getAsString();
        }

        LootContainerStateStore.open(new LootContainerStateStore.OpenSession(
            sessionId, sourceKind, grade, rows, cols, timeoutWallSecs
        ));

        return ServerDataDispatch.handled(type,
            "loot_container_open session=" + sessionId + " " + rows + "×" + cols);
    }

    private static ServerDataDispatch handleUpdate(String type, JsonObject payload) {
        Long sessionId = readLong(payload, "session_id");
        if (sessionId == null) {
            return ServerDataDispatch.noOp(type, "Ignoring loot_container_update: missing session_id");
        }
        return ServerDataDispatch.handled(type, "loot_container_update session=" + sessionId);
    }

    private static ServerDataDispatch handleClose(String type, JsonObject payload) {
        Long sessionId = readLong(payload, "session_id");
        if (sessionId == null) {
            return ServerDataDispatch.noOp(type, "Ignoring loot_container_close: missing session_id");
        }
        String reason = readString(payload, "reason", "unknown");
        LootContainerStateStore.close(sessionId, reason);
        return ServerDataDispatch.handled(type,
            "loot_container_close session=" + sessionId + " reason=" + reason);
    }

    private static Long readLong(JsonObject obj, String field) {
        JsonElement element = obj.get(field);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        return primitive.isNumber() ? primitive.getAsLong() : null;
    }

    private static int readInt(JsonObject obj, String field, int defaultValue) {
        Long value = readLong(obj, field);
        return value != null ? value.intValue() : defaultValue;
    }

    private static String readString(JsonObject obj, String field, String defaultValue) {
        JsonElement element = obj.get(field);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return defaultValue;
        }
        return element.getAsString();
    }
}
