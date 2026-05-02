package com.bong.client.network;

import com.bong.client.hud.SearchHudStateStore;
import com.bong.client.tsy.TsyContainerStateStore;
import com.bong.client.tsy.TsyContainerView;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

public final class ContainerInteractionHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        return switch (envelope.type()) {
            case "container_state" -> handleContainerState(envelope.type(), payload);
            case "search_started" -> handleSearchStarted(envelope.type(), payload);
            case "search_progress" -> handleSearchProgress(envelope.type(), payload);
            case "search_completed" -> handleSearchCompleted(envelope.type(), payload);
            case "search_aborted" -> handleSearchAborted(envelope.type(), payload);
            default -> ServerDataDispatch.noOp(
                envelope.type(),
                "Unsupported container interaction payload type " + envelope.type()
            );
        };
    }

    private static ServerDataDispatch handleContainerState(String type, JsonObject payload) {
        Long entityId = readLong(payload, "entity_id");
        double[] pos = readDoubleTriple(payload, "world_pos");
        if (entityId == null || pos == null) {
            return ServerDataDispatch.noOp(type, "Ignoring container_state: missing entity_id/world_pos");
        }
        TsyContainerStateStore.upsert(new TsyContainerView(
            entityId,
            readString(payload, "kind"),
            readString(payload, "family_id"),
            pos[0],
            pos[1],
            pos[2],
            readNullableString(payload, "locked"),
            readBoolean(payload, "depleted", false),
            readNullableString(payload, "searched_by_player_id")
        ));
        return ServerDataDispatch.handled(type, "Applied container state " + entityId);
    }

    private static ServerDataDispatch handleSearchStarted(String type, JsonObject payload) {
        Long entityId = readLong(payload, "container_entity_id");
        if (entityId == null) {
            return ServerDataDispatch.noOp(type, "Ignoring search_started: missing container_entity_id");
        }
        SearchHudStateStore.markStarted(kindLabel(entityId), readInt(payload, "required_ticks", 1));
        return ServerDataDispatch.handled(type, "Started search " + entityId);
    }

    private static ServerDataDispatch handleSearchProgress(String type, JsonObject payload) {
        Long entityId = readLong(payload, "container_entity_id");
        if (entityId == null) {
            return ServerDataDispatch.noOp(type, "Ignoring search_progress: missing container_entity_id");
        }
        SearchHudStateStore.markProgress(
            kindLabel(entityId),
            readInt(payload, "elapsed_ticks", 0),
            readInt(payload, "required_ticks", 1)
        );
        return ServerDataDispatch.handled(type, "Updated search progress " + entityId);
    }

    private static ServerDataDispatch handleSearchCompleted(String type, JsonObject payload) {
        Long entityId = readLong(payload, "container_entity_id");
        if (entityId == null) {
            return ServerDataDispatch.noOp(type, "Ignoring search_completed: missing container_entity_id");
        }
        TsyContainerView existing = TsyContainerStateStore.get(entityId);
        if (existing != null) {
            TsyContainerStateStore.upsert(new TsyContainerView(
                existing.entityId(),
                existing.kind(),
                existing.familyId(),
                existing.x(),
                existing.y(),
                existing.z(),
                existing.locked(),
                true,
                null
            ));
        }
        SearchHudStateStore.markCompleted(kindLabel(entityId));
        return ServerDataDispatch.handled(type, "Completed search " + entityId);
    }

    private static ServerDataDispatch handleSearchAborted(String type, JsonObject payload) {
        Long entityId = readLong(payload, "container_entity_id");
        if (entityId == null) {
            return ServerDataDispatch.noOp(type, "Ignoring search_aborted: missing container_entity_id");
        }
        SearchHudStateStore.markAborted(kindLabel(entityId), readString(payload, "reason"));
        return ServerDataDispatch.handled(type, "Aborted search " + entityId);
    }

    private static String kindLabel(long entityId) {
        TsyContainerView view = TsyContainerStateStore.get(entityId);
        return view == null ? "容器" : view.kindLabelZh();
    }

    private static String readString(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        return primitive != null && primitive.isString() ? primitive.getAsString() : "";
    }

    private static String readNullableString(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        return primitive != null && primitive.isString() ? primitive.getAsString() : null;
    }

    private static int readInt(JsonObject object, String fieldName, int fallback) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        return primitive != null && primitive.isNumber() ? primitive.getAsInt() : fallback;
    }

    private static Long readLong(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        return primitive != null && primitive.isNumber() ? primitive.getAsLong() : null;
    }

    private static boolean readBoolean(JsonObject object, String fieldName, boolean fallback) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        return primitive != null && primitive.isBoolean() ? primitive.getAsBoolean() : fallback;
    }

    private static JsonPrimitive readPrimitive(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }
        return element.getAsJsonPrimitive();
    }

    private static double[] readDoubleTriple(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || !element.isJsonArray()) {
            return null;
        }
        JsonArray array = element.getAsJsonArray();
        if (array.size() != 3) {
            return null;
        }
        double[] out = new double[3];
        for (int i = 0; i < 3; i++) {
            JsonElement value = array.get(i);
            if (!value.isJsonPrimitive() || !value.getAsJsonPrimitive().isNumber()) {
                return null;
            }
            out[i] = value.getAsDouble();
        }
        return out;
    }
}
