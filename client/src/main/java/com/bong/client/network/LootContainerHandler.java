package com.bong.client.network;

import com.bong.client.hud.LootContainerStateStore;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.JsonPrimitive;
import com.google.gson.JsonSyntaxException;

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

        SourceKindInfo sourceKind = parseSourceKind(payload.get("source_kind"));

        String containerId = "ext_" + sessionId;
        List<InventoryModel.GridEntry> items = parsePlacedItems(payload, containerId, rows, cols);

        LootContainerStateStore.open(new LootContainerStateStore.OpenSession(
            sessionId, sourceKind.kind(), sourceKind.grade(), rows, cols, timeoutWallSecs, items
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

    static SourceKindInfo parseSourceKind(JsonElement sourceKindEl) {
        if (sourceKindEl == null) {
            return new SourceKindInfo("unknown", "common");
        }
        // plan-wire-format-bridge-v1 P4/RC4：proto `source_kind` 是 `string` 字段，
        // 承载 server 侧 serde 外部标签 `LootContainerSourceKindV1` 的 JSON *再编码成字符串*
        // （proto_convert.rs: `serde_json::to_string(&o.source_kind)`），例如
        // supply_coffin → `"{\"supply_coffin\":{\"grade\":\"legendary\"}}"`，
        // dead_drop → `"\"dead_drop\""`。JsonFormat 不知道字符串里嵌了 JSON，只会把它
        // 当成普通字符串输出——下方 isJsonObject/variant-key 分支永远够不着，grade
        // 恒 "common"。在应用原有对象形态逻辑前先二次解码内嵌 JSON；解码失败（真正
        // 的裸字符串，例如手搓测试 fixture 或旧格式残留）则回退到原始文本。
        if (sourceKindEl.isJsonPrimitive() && sourceKindEl.getAsJsonPrimitive().isString()) {
            String raw = sourceKindEl.getAsString();
            // 只对"看起来像内嵌 JSON"的两种 server 编码形状（对象 `{...}` / 再引用的字符串
            // `"..."`）尝试二次解码；裸字符串（未来格式残留 / 测试直喂旧版 legacy 值）
            // 原样跳过，不依赖 Gson lenient 解析猜裸标识符。
            if (raw.startsWith("{") || raw.startsWith("\"")) {
                JsonElement decoded = tryParseInnerJson(raw);
                if (decoded != null) {
                    sourceKindEl = decoded;
                }
            }
        }
        if (sourceKindEl.isJsonPrimitive()) {
            return new SourceKindInfo(sourceKindEl.getAsString(), "common");
        }
        if (!sourceKindEl.isJsonObject()) {
            return new SourceKindInfo("unknown", "common");
        }
        JsonObject sk = sourceKindEl.getAsJsonObject();
        if (sk.has("kind")) {
            return new SourceKindInfo(
                readString(sk, "kind", "unknown"),
                readString(sk, "grade", "common")
            );
        }
        if (sk.has("supply_coffin") && sk.get("supply_coffin").isJsonObject()) {
            JsonObject supply = sk.getAsJsonObject("supply_coffin");
            return new SourceKindInfo("supply_coffin", readString(supply, "grade", "common"));
        }
        if (sk.has("storage_crate") && sk.get("storage_crate").isJsonObject()) {
            JsonObject crate = sk.getAsJsonObject("storage_crate");
            boolean isHerb = readBoolean(crate, "is_herb", false);
            return new SourceKindInfo("storage_crate", isHerb ? "herb" : "trade");
        }
        if (sk.has("dead_drop")) {
            return new SourceKindInfo("dead_drop", "common");
        }
        return new SourceKindInfo("unknown", "common");
    }

    record SourceKindInfo(String kind, String grade) {}

    /** 尝试把字符串当 JSON 再解析一次；失败（含空串/EOF/语法错误）返回 null 不抛异常。 */
    private static JsonElement tryParseInnerJson(String raw) {
        if (raw == null || raw.isEmpty()) {
            return null;
        }
        try {
            return JsonParser.parseString(raw);
        } catch (JsonSyntaxException | IllegalStateException e) {
            return null;
        }
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

    private static boolean readBoolean(JsonObject obj, String field, boolean defaultValue) {
        JsonElement element = obj.get(field);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return defaultValue;
        }
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        return primitive.isBoolean() ? primitive.getAsBoolean() : defaultValue;
    }
}
