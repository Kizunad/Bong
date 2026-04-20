package com.bong.client.network;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.state.DroppedItemStore;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import java.util.ArrayList;
import java.util.List;

public final class DroppedLootSyncHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonArray drops = readRequiredArray(envelope.payload(), "drops");
        if (drops == null) {
            return ServerDataDispatch.noOp(envelope.type(), "Ignoring dropped_loot_sync payload: missing drops array");
        }

        List<DroppedItemStore.Entry> entries = new ArrayList<>();
        for (JsonElement element : drops) {
            if (!element.isJsonObject()) {
                return ServerDataDispatch.noOp(envelope.type(), "Ignoring dropped_loot_sync payload: malformed drop entry");
            }
            DroppedItemStore.Entry entry = parseEntry(element.getAsJsonObject());
            if (entry == null) {
                return ServerDataDispatch.noOp(envelope.type(), "Ignoring dropped_loot_sync payload: invalid drop entry");
            }
            entries.add(entry);
        }

        DroppedItemStore.replaceAll(entries);
        return ServerDataDispatch.handled(envelope.type(), "Applied dropped_loot_sync with " + entries.size() + " entries");
    }

    private static DroppedItemStore.Entry parseEntry(JsonObject object) {
        Long instanceId = readRequiredLong(object, "instance_id");
        String sourceContainerId = readRequiredString(object, "source_container_id");
        Integer sourceRow = readRequiredInt(object, "source_row");
        Integer sourceCol = readRequiredInt(object, "source_col");
        JsonArray pos = readRequiredArray(object, "world_pos");
        InventoryItem item = parseInventoryItem(readRequiredObject(object, "item"));
        if (instanceId == null || sourceContainerId == null || sourceRow == null || sourceCol == null || pos == null || item == null) {
            return null;
        }
        Double x = readRequiredDouble(pos.get(0));
        Double y = readRequiredDouble(pos.get(1));
        Double z = readRequiredDouble(pos.get(2));
        if (x == null || y == null || z == null) {
            return null;
        }
        return new DroppedItemStore.Entry(instanceId, sourceContainerId, sourceRow, sourceCol, x, y, z, item);
    }

    private static InventoryItem parseInventoryItem(JsonObject itemObject) {
        if (itemObject == null) return null;
        Long instanceId = readRequiredLong(itemObject, "instance_id");
        String itemId = readRequiredString(itemObject, "item_id");
        String displayName = readRequiredString(itemObject, "display_name");
        Integer gridWidth = readRequiredInt(itemObject, "grid_width");
        Integer gridHeight = readRequiredInt(itemObject, "grid_height");
        Double weight = readRequiredDouble(itemObject, "weight");
        String rarity = readRequiredString(itemObject, "rarity");
        String description = readRequiredStringAllowEmpty(itemObject, "description");
        Integer stackCount = readRequiredInt(itemObject, "stack_count");
        Double spiritQuality = readRequiredDouble(itemObject, "spirit_quality");
        Double durability = readRequiredDouble(itemObject, "durability");
        if (instanceId == null || itemId == null || displayName == null || gridWidth == null || gridHeight == null
            || weight == null || rarity == null || description == null || stackCount == null
            || spiritQuality == null || durability == null) {
            return null;
        }
        return InventoryItem.createFull(
            instanceId, itemId, displayName, gridWidth, gridHeight,
            weight, rarity, description, stackCount, spiritQuality, durability
        );
    }

    private static JsonObject readRequiredObject(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        return element != null && element.isJsonObject() ? element.getAsJsonObject() : null;
    }

    private static JsonArray readRequiredArray(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        return element != null && element.isJsonArray() ? element.getAsJsonArray() : null;
    }

    private static String readRequiredString(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) return null;
        return element.getAsString();
    }

    private static String readRequiredStringAllowEmpty(JsonObject object, String fieldName) {
        return readRequiredString(object, fieldName);
    }

    private static Long readRequiredLong(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) return null;
        try {
            return element.getAsLong();
        } catch (RuntimeException e) {
            return null;
        }
    }

    private static Integer readRequiredInt(JsonObject object, String fieldName) {
        Long value = readRequiredLong(object, fieldName);
        return value == null || value > Integer.MAX_VALUE ? null : value.intValue();
    }

    private static Double readRequiredDouble(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        return readRequiredDouble(element);
    }

    private static Double readRequiredDouble(JsonElement element) {
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) return null;
        try {
            return element.getAsDouble();
        } catch (RuntimeException e) {
            return null;
        }
    }
}
