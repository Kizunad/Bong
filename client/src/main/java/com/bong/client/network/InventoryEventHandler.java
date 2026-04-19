package com.bong.client.network;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.Map;
import java.util.regex.Pattern;

public final class InventoryEventHandler implements ServerDataHandler {
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

    private sealed interface Location {}
    private record ContainerLoc(String containerId, int row, int col) implements Location {}
    private record EquipLoc(EquipSlotType slot) implements Location {}
    private record HotbarLoc(int index) implements Location {}

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        if (!InventoryStateStore.isAuthoritativeLoaded()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring inventory_event payload because authoritative inventory snapshot is not loaded yet"
            );
        }

        JsonObject payload = envelope.payload();
        Long revision = readRequiredLong(payload, "revision");
        String kind = readRequiredString(payload, "kind");
        Long instanceId = readRequiredLong(payload, "instance_id");
        if (revision == null || kind == null || instanceId == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring inventory_event payload because required fields are missing or invalid"
            );
        }

        long currentRevision = InventoryStateStore.revision();
        if (revision < currentRevision) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring inventory_event payload because revision " + revision
                    + " is stale (store revision " + currentRevision + ")"
            );
        }

        InventoryModel current = InventoryStateStore.snapshot();
        InventoryModel next;
        switch (kind) {
            case "moved" -> {
                Location from = parseLocation(readRequiredObject(payload, "from"));
                Location to = parseLocation(readRequiredObject(payload, "to"));
                if (from == null || to == null) {
                    return ServerDataDispatch.noOp(envelope.type(),
                        "Ignoring inventory_event 'moved' payload: invalid from/to location");
                }
                next = applyMoved(current, instanceId, from, to);
            }
            case "stack_changed" -> {
                Long stackCount = readRequiredLong(payload, "stack_count");
                if (stackCount == null || stackCount < 1 || stackCount > Integer.MAX_VALUE) {
                    return ServerDataDispatch.noOp(envelope.type(),
                        "Ignoring inventory_event 'stack_changed' payload: invalid stack_count");
                }
                next = applyItemReplace(current, instanceId,
                    item -> withStack(item, stackCount.intValue()));
            }
            case "durability_changed" -> {
                Double durability = readRequiredDouble(payload, "durability");
                if (durability == null || durability < 0.0 || durability > 1.0) {
                    return ServerDataDispatch.noOp(envelope.type(),
                        "Ignoring inventory_event 'durability_changed' payload: invalid durability");
                }
                next = applyItemReplace(current, instanceId,
                    item -> withDurability(item, durability));
            }
            default -> {
                return ServerDataDispatch.noOp(envelope.type(),
                    "Ignoring inventory_event payload because kind '" + kind + "' is unsupported");
            }
        }

        if (next == null) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring inventory_event '" + kind + "' for instance_id " + instanceId
                    + ": item not found in current snapshot");
        }

        InventoryStateStore.applyAuthoritativeSnapshot(next, revision);
        return ServerDataDispatch.handled(envelope.type(),
            "Applied inventory_event '" + kind + "' (instance_id " + instanceId
                + ", revision " + revision + ")");
    }

    // ─── Mutation helpers ───────────────────────────────────────────────────

    private static InventoryModel applyMoved(InventoryModel current, long instanceId, Location from, Location to) {
        InventoryItem item = findItem(current, instanceId);
        if (item == null) return null;
        // 注意：不要在这里 reject 当 from 不匹配——客户端拖拽流是「乐观先动 + 派发 intent」，
        // 等 server 回推 moved 时 item 已经在 to。原本的 from 校验会让所有 client-initiated
        // move 的回推被丢弃 → InspectScreen 永远不知道 server 同意了。
        // rebuildWith 本身按 instance_id 找位置 + 拔出 + 重放到 to，幂等。
        // 校验 to 已被占用的合法性由 server 的 apply_inventory_move 把关，client 信任之。
        return rebuildWith(current, instanceId, /* skip */ null, item, to);
    }

    /** Replace the item identified by {@code instanceId} with the result of {@code transform}. */
    private static InventoryModel applyItemReplace(InventoryModel current, long instanceId,
                                                    java.util.function.Function<InventoryItem, InventoryItem> transform) {
        InventoryItem item = findItem(current, instanceId);
        if (item == null) return null;
        InventoryItem replacement = transform.apply(item);
        return rebuildWith(current, instanceId, replacement, null, null);
    }

    /**
     * Rebuild model:
     *   - replacement != null AND target == null → replace item in place
     *   - replacement == null AND target != null → move item to target
     */
    private static InventoryModel rebuildWith(InventoryModel current, long instanceId,
                                               InventoryItem replacement, InventoryItem moveItem, Location target) {
        InventoryModel.Builder builder = InventoryModel.builder()
            .containers(current.containers())
            .weight(current.currentWeight(), current.maxWeight())
            .boneCoins(current.boneCoins())
            .cultivation(current.realm(), current.qiCurrent(), current.qiMax(), current.bodyLevel());

        // Container grid items.
        for (InventoryModel.GridEntry entry : current.gridItems()) {
            InventoryItem entryItem = entry.item();
            if (entryItem.instanceId() == instanceId) {
                if (replacement != null) {
                    builder.gridItem(replacement, entry.containerId(), entry.row(), entry.col());
                }
                // moved → skip; will be re-placed at target below.
            } else {
                builder.gridItem(entryItem, entry.containerId(), entry.row(), entry.col());
            }
        }

        // Equipped.
        for (Map.Entry<EquipSlotType, InventoryItem> e : current.equipped().entrySet()) {
            InventoryItem slotItem = e.getValue();
            if (slotItem != null && slotItem.instanceId() == instanceId) {
                if (replacement != null) {
                    builder.equip(e.getKey(), replacement);
                }
            } else {
                builder.equip(e.getKey(), slotItem);
            }
        }

        // Hotbar.
        for (int i = 0; i < current.hotbar().size(); i++) {
            InventoryItem h = current.hotbar().get(i);
            if (h != null && h.instanceId() == instanceId) {
                if (replacement != null) {
                    builder.hotbar(i, replacement);
                }
            } else if (h != null) {
                builder.hotbar(i, h);
            }
        }

        // Place moved item at target.
        if (moveItem != null && target != null) {
            placeAt(builder, moveItem, target);
        }

        return builder.build();
    }

    private static void placeAt(InventoryModel.Builder builder, InventoryItem item, Location target) {
        if (target instanceof ContainerLoc loc) {
            builder.gridItem(item, loc.containerId(), loc.row(), loc.col());
        } else if (target instanceof EquipLoc loc) {
            builder.equip(loc.slot(), item);
        } else if (target instanceof HotbarLoc loc) {
            builder.hotbar(loc.index(), item);
        }
    }

    private static InventoryItem findItem(InventoryModel model, long instanceId) {
        for (InventoryModel.GridEntry entry : model.gridItems()) {
            if (entry.item().instanceId() == instanceId) return entry.item();
        }
        for (InventoryItem item : model.equipped().values()) {
            if (item != null && item.instanceId() == instanceId) return item;
        }
        for (InventoryItem item : model.hotbar()) {
            if (item != null && item.instanceId() == instanceId) return item;
        }
        return null;
    }

    private static InventoryItem withStack(InventoryItem item, int stackCount) {
        return InventoryItem.createFull(
            item.instanceId(), item.itemId(), item.displayName(),
            item.gridWidth(), item.gridHeight(), item.weight(),
            item.rarity(), item.description(),
            stackCount, item.spiritQuality(), item.durability()
        );
    }

    private static InventoryItem withDurability(InventoryItem item, double durability) {
        return InventoryItem.createFull(
            item.instanceId(), item.itemId(), item.displayName(),
            item.gridWidth(), item.gridHeight(), item.weight(),
            item.rarity(), item.description(),
            item.stackCount(), item.spiritQuality(), durability
        );
    }

    // ─── Location parsing ───────────────────────────────────────────────────

    private static Location parseLocation(JsonObject obj) {
        if (obj == null) return null;
        String kind = readRequiredString(obj, "kind");
        if (kind == null) return null;
        return switch (kind) {
            case "container" -> {
                String containerId = readRequiredString(obj, "container_id");
                Long row = readRequiredLong(obj, "row");
                Long col = readRequiredLong(obj, "col");
                if (containerId == null || row == null || col == null
                    || row > Integer.MAX_VALUE || col > Integer.MAX_VALUE) {
                    yield null;
                }
                yield new ContainerLoc(containerId, row.intValue(), col.intValue());
            }
            case "equip" -> {
                String slotName = readRequiredString(obj, "slot");
                if (slotName == null) yield null;
                EquipSlotType slot = EQUIP_SLOT_BY_WIRE_NAME.get(slotName);
                yield slot == null ? null : new EquipLoc(slot);
            }
            case "hotbar" -> {
                Long index = readRequiredLong(obj, "index");
                if (index == null || index >= InventoryModel.HOTBAR_SIZE) yield null;
                yield new HotbarLoc(index.intValue());
            }
            default -> null;
        };
    }

    // ─── JSON helpers ───────────────────────────────────────────────────────

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

    private static JsonObject readRequiredObject(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonObject()) {
            return null;
        }
        return element.getAsJsonObject();
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
