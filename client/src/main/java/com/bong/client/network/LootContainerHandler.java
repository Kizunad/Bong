package com.bong.client.network;

import com.bong.client.hud.LootContainerStateStore;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.ArrayList;
import java.util.List;

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

        String containerId = "ext_" + sessionId;
        List<InventoryModel.GridEntry> items = parsePlacedItems(payload, containerId, rows, cols);

        LootContainerStateStore.open(new LootContainerStateStore.OpenSession(
            sessionId, sourceKind, grade, rows, cols, timeoutWallSecs, items
        ));

        return ServerDataDispatch.handled(type,
            "loot_container_open session=" + sessionId + " " + rows + "×" + cols
                + " items=" + items.size());
    }

    private static ServerDataDispatch handleUpdate(String type, JsonObject payload) {
        Long sessionId = readLong(payload, "session_id");
        if (sessionId == null) {
            return ServerDataDispatch.noOp(type, "Ignoring loot_container_update: missing session_id");
        }

        LootContainerStateStore.Session current = LootContainerStateStore.current();
        if (!(current instanceof LootContainerStateStore.OpenSession open)) {
            return ServerDataDispatch.noOp(type, "Ignoring loot_container_update: no open session");
        }

        String containerId = "ext_" + sessionId;
        List<InventoryModel.GridEntry> items = parsePlacedItems(
            payload, containerId, open.rows(), open.cols()
        );
        LootContainerStateStore.update(sessionId, items);

        return ServerDataDispatch.handled(type,
            "loot_container_update session=" + sessionId + " items=" + items.size());
    }

    private static ServerDataDispatch handleClose(String type, JsonObject payload) {
        Long sessionId = readLong(payload, "session_id");
        if (sessionId == null) {
            return ServerDataDispatch.noOp(type, "Ignoring loot_container_close: missing session_id");
        }
        String reason = readString(payload, "reason", "unknown");
        if (!isKnownCloseReason(reason)) {
            return ServerDataDispatch.noOp(type,
                "Ignoring loot_container_close: unknown reason " + reason);
        }
        LootContainerStateStore.close(sessionId, reason);
        return ServerDataDispatch.handled(type,
            "loot_container_close session=" + sessionId + " reason=" + reason);
    }

    private static boolean isKnownCloseReason(String reason) {
        return switch (reason) {
            case "timeout", "distance", "player_closed", "coffin_destroyed", "container_destroyed" -> true;
            default -> false;
        };
    }

    static List<InventoryModel.GridEntry> parsePlacedItems(
        JsonObject payload, String containerId, int maxRows, int maxCols
    ) {
        JsonElement itemsEl = payload.get("placed_items");
        if (itemsEl == null || !itemsEl.isJsonArray()) {
            return List.of();
        }
        JsonArray arr = itemsEl.getAsJsonArray();
        List<InventoryModel.GridEntry> entries = new ArrayList<>(arr.size());
        for (JsonElement el : arr) {
            if (el == null || !el.isJsonObject()) continue;
            JsonObject obj = el.getAsJsonObject();
            Integer row = readIntOrNull(obj, "row");
            Integer col = readIntOrNull(obj, "col");
            JsonObject itemObj = obj.has("item") && obj.get("item").isJsonObject()
                ? obj.getAsJsonObject("item") : null;
            if (row == null || col == null || itemObj == null) continue;
            InventoryItem item = InventorySnapshotHandler.parseInventoryItem(itemObj);
            if (item == null) continue;
            if (row < 0 || col < 0 || row + item.gridHeight() > maxRows || col + item.gridWidth() > maxCols) continue;
            entries.add(new InventoryModel.GridEntry(item, containerId, row, col));
        }
        return entries;
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
        if (value == null) return defaultValue;
        if (value < Integer.MIN_VALUE || value > Integer.MAX_VALUE) return defaultValue;
        return value.intValue();
    }

    private static Integer readIntOrNull(JsonObject obj, String field) {
        Long value = readLong(obj, field);
        if (value != null) {
            if (value < Integer.MIN_VALUE || value > Integer.MAX_VALUE) return null;
            return value.intValue();
        }
        return null;
    }

    private static String readString(JsonObject obj, String field, String defaultValue) {
        JsonElement element = obj.get(field);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return defaultValue;
        }
        return element.getAsString();
    }
}
