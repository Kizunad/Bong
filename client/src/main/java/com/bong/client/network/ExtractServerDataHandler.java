package com.bong.client.network;

import com.bong.client.tsy.ExtractStateStore;
import com.bong.client.tsy.RiftPortalView;
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
                double[] pos = readFlatVec3(payload, "world_pos");
                if (entityId == null || pos == null) {
                    return ServerDataDispatch.noOp(envelope.type(), "Ignoring rift_portal_state: missing entity_id/world_pos");
                }
                ExtractStateStore.upsertPortal(new RiftPortalView(
                    entityId,
                    readString(payload, "kind"),
                    readString(payload, "direction"),
                    readString(payload, "family_id"),
                    pos[0], pos[1], pos[2],
                    readDouble(payload, "trigger_radius", ExtractStateStore.PORTAL_INTERACT_RADIUS),
                    readInt(payload, "current_extract_ticks", 0),
                    readLong(payload, "activation_window_end")
                ));
                return ServerDataDispatch.handled(envelope.type(), "Applied rift portal state " + entityId);
            }
            case "rift_portal_removed" -> {
                Long entityId = readLong(payload, "entity_id");
                if (entityId == null) {
                    return ServerDataDispatch.noOp(envelope.type(), "Ignoring rift_portal_removed: missing entity_id");
                }
                ExtractStateStore.removePortal(entityId);
                return ServerDataDispatch.handled(envelope.type(), "Removed rift portal " + entityId);
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

    private static double readDouble(JsonObject object, String fieldName, double fallback) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        return primitive != null && primitive.isNumber() ? primitive.getAsDouble() : fallback;
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

    /**
     * proto→legacy-JSON 桥不做 flat→array 重塑（见 ProtoServerDataBridge 类注释）：
     * proto {@code [f64;3]} 坐标一律拆成 {@code <prefix>_x/_y/_z} 三个 flat 字段落地。
     * 生产走 protobuf 时读数组形状（旧 {@code readDoubleTriple}）永远拿 null →
     * 静默 noOp 丢数据。故这里改读 flat 三字段，任一缺失/非数字即返回 null（保持
     * "null → noOp" 的拒绝语义不变）。
     */
    private static double[] readFlatVec3(JsonObject object, String prefix) {
        Double x = readDoubleOrNull(object, prefix + "_x");
        Double y = readDoubleOrNull(object, prefix + "_y");
        Double z = readDoubleOrNull(object, prefix + "_z");
        if (x == null || y == null || z == null) {
            return null;
        }
        return new double[] {x, y, z};
    }

    private static Double readDoubleOrNull(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        return primitive != null && primitive.isNumber() ? primitive.getAsDouble() : null;
    }
}
