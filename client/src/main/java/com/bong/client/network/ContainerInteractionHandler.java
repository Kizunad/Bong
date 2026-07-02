package com.bong.client.network;

import com.bong.client.hud.SearchHudStateStore;
import com.bong.client.tsy.TsyContainerStateStore;
import com.bong.client.tsy.TsyContainerView;
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
        // ContainerStateProto (proto/bong/envelope.proto) carries world_pos as three flat
        // double fields (world_pos_x/world_pos_y/world_pos_z), not a JSON array — proto [f64;3]
        // fields are always split flat on the wire. ProtoServerDataBridge does not reshape
        // flat fields back into arrays, so reading "world_pos" as an array here silently
        // returned null under the production (--release) proto wire, dropping every
        // container_state update.
        double[] pos = readFlatVec3(payload, "world_pos");
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

    /**
     * Reads a flattened {@code [f64;3]} coordinate as three sibling fields
     * {@code <fieldPrefix>_x/_y/_z} (e.g. {@code world_pos_x/world_pos_y/world_pos_z}),
     * matching how {@code ContainerStateProto} lays out {@code world_pos} on the wire.
     * Returns {@code null} if any of the three fields is missing or not a number.
     */
    private static double[] readFlatVec3(JsonObject object, String fieldPrefix) {
        Double x = readDouble(object, fieldPrefix + "_x");
        Double y = readDouble(object, fieldPrefix + "_y");
        Double z = readDouble(object, fieldPrefix + "_z");
        if (x == null || y == null || z == null) {
            return null;
        }
        return new double[] {x, y, z};
    }

    private static Double readDouble(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        return primitive != null && primitive.isNumber() ? primitive.getAsDouble() : null;
    }
}
