package com.bong.client.network;

import com.bong.client.tsy.ExtractStateStore;
import com.bong.client.tsy.RiftPortalView;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

public final class ExtractServerDataHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        long nowMs = System.currentTimeMillis();
        switch (envelope.type()) {
            case "rift_portal_state" -> {
                Long entityId = readLong(payload, "entity_id");
                double[] pos = readDoubleTriple(payload, "world_pos");
                if (entityId == null || pos == null) {
                    return ServerDataDispatch.noOp(envelope.type(), "Ignoring rift_portal_state: missing entity_id/world_pos");
                }
                ExtractStateStore.upsertPortal(new RiftPortalView(
                    entityId,
                    readString(payload, "kind"),
                    readString(payload, "family_id"),
                    pos[0], pos[1], pos[2],
                    readInt(payload, "current_extract_ticks", 0),
                    readLong(payload, "activation_window_end")
                ));
                return ServerDataDispatch.handled(envelope.type(), "Applied rift portal state " + entityId);
            }
            case "extract_started" -> {
                Long portalId = readLong(payload, "portal_entity_id");
                if (portalId == null) {
                    return ServerDataDispatch.noOp(envelope.type(), "Ignoring extract_started: missing portal_entity_id");
                }
                ExtractStateStore.markStarted(
                    portalId,
                    readString(payload, "portal_kind"),
                    readInt(payload, "required_ticks", 0),
                    nowMs
                );
                return ServerDataDispatch.handled(envelope.type(), "Started extract via portal " + portalId);
            }
            case "extract_progress" -> {
                Long portalId = readLong(payload, "portal_entity_id");
                if (portalId == null) {
                    return ServerDataDispatch.noOp(envelope.type(), "Ignoring extract_progress: missing portal_entity_id");
                }
                ExtractStateStore.markProgress(
                    portalId,
                    readInt(payload, "elapsed_ticks", 0),
                    readInt(payload, "required_ticks", 0),
                    nowMs
                );
                return ServerDataDispatch.handled(envelope.type(), "Updated extract progress " + portalId);
            }
            case "extract_completed" -> {
                ExtractStateStore.markCompleted(readString(payload, "family_id"), nowMs);
                return ServerDataDispatch.handled(envelope.type(), "Completed extract");
            }
            case "extract_aborted" -> {
                ExtractStateStore.markAborted(readString(payload, "reason"), nowMs);
                return ServerDataDispatch.handled(envelope.type(), "Aborted extract");
            }
            case "extract_failed" -> {
                ExtractStateStore.markFailed(readString(payload, "reason"), nowMs);
                return ServerDataDispatch.handled(envelope.type(), "Failed extract");
            }
            case "tsy_collapse_started_ipc" -> {
                ExtractStateStore.markCollapseStarted(
                    readString(payload, "family_id"),
                    readInt(payload, "remaining_ticks", 0),
                    nowMs
                );
                return ServerDataDispatch.handled(envelope.type(), "Started TSY collapse HUD countdown");
            }
            default -> {
                return ServerDataDispatch.noOp(envelope.type(), "Unsupported extract payload type " + envelope.type());
            }
        }
    }

    private static String readString(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        return primitive != null && primitive.isString() ? primitive.getAsString() : "";
    }

    private static int readInt(JsonObject object, String fieldName, int fallback) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        return primitive != null && primitive.isNumber() ? primitive.getAsInt() : fallback;
    }

    private static Long readLong(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        return primitive != null && primitive.isNumber() ? primitive.getAsLong() : null;
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
