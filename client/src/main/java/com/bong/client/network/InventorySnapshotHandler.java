package com.bong.client.network;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.ArrayList;
import java.util.EnumMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.regex.Pattern;

public final class InventorySnapshotHandler implements ServerDataHandler {
    private static final long JS_SAFE_INTEGER_MAX = 9_007_199_254_740_991L;
    private static final Pattern INTEGER_TOKEN_PATTERN = Pattern.compile("-?(0|[1-9]\\d*)");

    private static final Map<String, EquipSlotType> EQUIP_SLOT_BY_WIRE_NAME = Map.of(
        "head", EquipSlotType.HEAD,
        "chest", EquipSlotType.CHEST,
        "legs", EquipSlotType.LEGS,
        "feet", EquipSlotType.FEET,
        "main_hand", EquipSlotType.MAIN_HAND,
        "off_hand", EquipSlotType.OFF_HAND,
        "two_hand", EquipSlotType.TWO_HAND
    );

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        Long revision = readRequiredLong(payload, "revision");
        JsonArray containerElements = readRequiredArray(payload, "containers");
        JsonArray placedItemElements = readRequiredArray(payload, "placed_items");
        JsonObject equippedObject = readRequiredObject(payload, "equipped");
        JsonArray hotbarElements = readRequiredArray(payload, "hotbar");
        JsonObject weightObject = readRequiredObject(payload, "weight");
        Long boneCoins = readRequiredLong(payload, "bone_coins");
        String realm = readRequiredString(payload, "realm");
        Double qiCurrent = readRequiredDouble(payload, "qi_current");
        Double qiMax = readRequiredDouble(payload, "qi_max");
        Double bodyLevel = readRequiredDouble(payload, "body_level");

        if (revision == null || containerElements == null || placedItemElements == null
            || equippedObject == null || hotbarElements == null || weightObject == null
            || boneCoins == null || realm == null || qiCurrent == null
            || qiMax == null || bodyLevel == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring inventory_snapshot payload: missing or invalid required root field(s)"
            );
        }

        Double currentWeight = readRequiredDouble(weightObject, "current");
        Double maxWeight = readRequiredDouble(weightObject, "max");
        if (currentWeight == null || maxWeight == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring inventory_snapshot payload: missing or invalid weight.current/weight.max"
            );
        }

        List<InventoryModel.ContainerDef> containers = parseContainers(containerElements);
        if (containers == null || containers.isEmpty()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring inventory_snapshot payload: containers are missing or invalid"
            );
        }

        Set<String> containerIds = new HashSet<>();
        for (InventoryModel.ContainerDef container : containers) {
            containerIds.add(container.id());
        }

        List<InventoryModel.GridEntry> gridEntries = parsePlacedItems(placedItemElements, containerIds);
        if (gridEntries == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring inventory_snapshot payload: placed_items are missing or invalid"
            );
        }

        EnumMap<EquipSlotType, InventoryItem> equipped = parseEquipped(equippedObject);
        if (equipped == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring inventory_snapshot payload: equipped is missing or invalid"
            );
        }

        List<InventoryItem> hotbarItems = parseHotbar(hotbarElements);
        if (hotbarItems == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring inventory_snapshot payload: hotbar is missing or invalid"
            );
        }

        InventoryModel.Builder builder = InventoryModel.builder()
            .containers(containers)
            .weight(currentWeight, maxWeight)
            .boneCoins(boneCoins)
            .cultivation(realm, qiCurrent, qiMax, bodyLevel);

        for (InventoryModel.GridEntry gridEntry : gridEntries) {
            builder.gridItem(gridEntry.item(), gridEntry.containerId(), gridEntry.row(), gridEntry.col());
        }
        for (Map.Entry<EquipSlotType, InventoryItem> entry : equipped.entrySet()) {
            builder.equip(entry.getKey(), entry.getValue());
        }
        for (int index = 0; index < hotbarItems.size(); index++) {
            InventoryItem item = hotbarItems.get(index);
            if (item != null) {
                builder.hotbar(index, item);
            }
        }

        InventoryModel model = builder.build();
        InventoryStateStore.applyAuthoritativeSnapshot(model, revision);
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied inventory_snapshot revision " + revision + " to InventoryStateStore"
        );
    }

    private static List<InventoryModel.ContainerDef> parseContainers(JsonArray containerElements) {
        List<InventoryModel.ContainerDef> containers = new ArrayList<>(containerElements.size());
        for (JsonElement containerElement : containerElements) {
            if (containerElement == null || containerElement.isJsonNull() || !containerElement.isJsonObject()) {
                return null;
            }

            JsonObject containerObject = containerElement.getAsJsonObject();
            String id = readRequiredString(containerObject, "id");
            String name = readRequiredString(containerObject, "name");
            Integer rows = readRequiredInt(containerObject, "rows");
            Integer cols = readRequiredInt(containerObject, "cols");
            if (id == null || name == null || rows == null || cols == null || rows <= 0 || cols <= 0) {
                return null;
            }

            try {
                containers.add(new InventoryModel.ContainerDef(id, name, rows, cols));
            } catch (IllegalArgumentException exception) {
                return null;
            }
        }

        return containers;
    }

    private static List<InventoryModel.GridEntry> parsePlacedItems(JsonArray placedItemElements, Set<String> containerIds) {
        List<InventoryModel.GridEntry> gridEntries = new ArrayList<>(placedItemElements.size());
        for (JsonElement placedItemElement : placedItemElements) {
            if (placedItemElement == null || placedItemElement.isJsonNull() || !placedItemElement.isJsonObject()) {
                return null;
            }

            JsonObject placedItemObject = placedItemElement.getAsJsonObject();
            String containerId = readRequiredString(placedItemObject, "container_id");
            Integer row = readRequiredInt(placedItemObject, "row");
            Integer col = readRequiredInt(placedItemObject, "col");
            JsonObject itemObject = readRequiredObject(placedItemObject, "item");
            if (containerId == null || row == null || col == null || itemObject == null || row < 0 || col < 0) {
                return null;
            }
            if (!containerIds.contains(containerId)) {
                return null;
            }

            InventoryItem item = parseInventoryItem(itemObject);
            if (item == null) {
                return null;
            }

            try {
                gridEntries.add(new InventoryModel.GridEntry(item, containerId, row, col));
            } catch (IllegalArgumentException exception) {
                return null;
            }
        }

        return gridEntries;
    }

    private static EnumMap<EquipSlotType, InventoryItem> parseEquipped(JsonObject equippedObject) {
        EnumMap<EquipSlotType, InventoryItem> equipped = new EnumMap<>(EquipSlotType.class);
        for (Map.Entry<String, EquipSlotType> slotEntry : EQUIP_SLOT_BY_WIRE_NAME.entrySet()) {
            JsonElement itemElement = equippedObject.get(slotEntry.getKey());
            if (itemElement == null) {
                return null;
            }
            if (itemElement.isJsonNull()) {
                continue;
            }
            if (!itemElement.isJsonObject()) {
                return null;
            }

            InventoryItem item = parseInventoryItem(itemElement.getAsJsonObject());
            if (item == null) {
                return null;
            }
            equipped.put(slotEntry.getValue(), item);
        }

        return equipped;
    }

    private static List<InventoryItem> parseHotbar(JsonArray hotbarElements) {
        if (hotbarElements.size() != InventoryModel.HOTBAR_SIZE) {
            return null;
        }

        List<InventoryItem> hotbarItems = new ArrayList<>(InventoryModel.HOTBAR_SIZE);
        for (JsonElement hotbarElement : hotbarElements) {
            if (hotbarElement == null || hotbarElement.isJsonNull()) {
                hotbarItems.add(null);
                continue;
            }
            if (!hotbarElement.isJsonObject()) {
                return null;
            }

            InventoryItem item = parseInventoryItem(hotbarElement.getAsJsonObject());
            if (item == null) {
                return null;
            }
            hotbarItems.add(item);
        }

        return hotbarItems;
    }

    private static InventoryItem parseInventoryItem(JsonObject itemObject) {
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

        if (instanceId == null || itemId == null || displayName == null
            || gridWidth == null || gridHeight == null || weight == null
            || rarity == null || description == null || stackCount == null
            || spiritQuality == null || durability == null
            || gridWidth < 1 || gridHeight < 1 || weight < 0.0 || stackCount < 1
            || spiritQuality < 0.0 || spiritQuality > 1.0
            || durability < 0.0 || durability > 1.0) {
            return null;
        }

        return InventoryItem.createFull(
            instanceId,
            itemId,
            displayName,
            gridWidth,
            gridHeight,
            weight,
            rarity,
            description,
            stackCount,
            spiritQuality,
            durability
        );
    }

    private static JsonArray readRequiredArray(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonArray()) {
            return null;
        }
        return element.getAsJsonArray();
    }

    private static JsonObject readRequiredObject(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonObject()) {
            return null;
        }
        return element.getAsJsonObject();
    }

    private static String readRequiredString(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }

        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isString()) {
            return null;
        }

        String value = primitive.getAsString().trim();
        return value.isEmpty() ? null : value;
    }

    private static String readRequiredStringAllowEmpty(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }

        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isString()) {
            return null;
        }

        return primitive.getAsString();
    }

    private static Double readRequiredDouble(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }

        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isNumber()) {
            return null;
        }

        double value = primitive.getAsDouble();
        return Double.isFinite(value) ? value : null;
    }

    private static Integer readRequiredInt(JsonObject object, String fieldName) {
        Long value = readRequiredLong(object, fieldName);
        if (value == null || value > Integer.MAX_VALUE) {
            return null;
        }
        return value.intValue();
    }

    private static Long readRequiredLong(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }

        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isNumber()) {
            return null;
        }

        String token = primitive.getAsString();
        if (!INTEGER_TOKEN_PATTERN.matcher(token).matches()) {
            return null;
        }

        long value;
        try {
            value = Long.parseLong(token);
        } catch (NumberFormatException exception) {
            return null;
        }

        if (value < 0 || value > JS_SAFE_INTEGER_MAX) {
            return null;
        }

        return value;
    }
}
