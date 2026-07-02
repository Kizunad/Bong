package com.bong.client.network;

import com.bong.client.armor.ArmorBreakParticles;
import com.bong.client.armor.ArmorTintRegistry;
import com.bong.client.combat.ArmorProfileStore;
import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.DroppedItemStore;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.state.VisualEffectState;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import net.minecraft.entity.EquipmentSlot;

import java.util.Map;
import java.util.regex.Pattern;

public final class InventoryEventHandler implements ServerDataHandler {
    private static final long JS_SAFE_INTEGER_MAX = 9_007_199_254_740_991L;
    private static final Pattern INTEGER_TOKEN_PATTERN = Pattern.compile("-?(0|[1-9]\\d*)");

    // plan-layered-equip-v1 P4（决议 #17）：与 InventorySnapshotHandler 同槽集对齐。
    private static final Map<String, EquipSlotType> EQUIP_SLOT_BY_WIRE_NAME = Map.ofEntries(
        Map.entry("head", EquipSlotType.HEAD),
        Map.entry("chest", EquipSlotType.CHEST),
        Map.entry("legs", EquipSlotType.LEGS),
        Map.entry("feet", EquipSlotType.FEET),
        Map.entry("main_hand", EquipSlotType.MAIN_HAND),
        Map.entry("off_hand", EquipSlotType.OFF_HAND),
        Map.entry("extra_hand_0", EquipSlotType.EXTRA_HAND_0),
        Map.entry("extra_hand_1", EquipSlotType.EXTRA_HAND_1)
    );

    private sealed interface Location {}
    private record ContainerLoc(String containerId, int row, int col) implements Location {}
    private record EquipLoc(EquipSlotType slot) implements Location {}
    private record HotbarLoc(int index) implements Location {}
    private record WorldPos(double x, double y, double z) {}

    private static final int ARMOR_BROKEN_TOAST_COLOR = 0xFFC04040;
    private static final long ARMOR_BROKEN_TOAST_DURATION_MS = 1200L;
    private static final int ARMOR_WARNING_TOAST_COLOR = 0xFFE05050;
    private static final long ARMOR_WARNING_TOAST_DURATION_MS = 1400L;
    private static final double ARMOR_LOW_DURABILITY_THRESHOLD = 0.20;
    private static final long ARMOR_EQUIP_FLASH_DURATION_MS = 100L;
    private static final long ARMOR_LOW_DURABILITY_FLASH_DURATION_MS = 700L;
    private static final long ARMOR_BREAK_FLASH_DURATION_MS = 300L;

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
        ServerDataDispatch.ToastSpec alertToast = null;
        VisualEffectState visualEffectState = VisualEffectState.none();
        switch (kind) {
            case "moved" -> {
                Location from = parseLocation(readRequiredObject(payload, "from"));
                Location to = parseLocation(readRequiredObject(payload, "to"));
                if (from == null || to == null) {
                    return ServerDataDispatch.noOp(envelope.type(),
                        "Ignoring inventory_event 'moved' payload: invalid from/to location");
                }
                InventoryItem item = findItem(current, instanceId);
                next = applyMoved(current, instanceId, from, to);
                if (next != null && item != null && isArmorEquipMove(item, to)) {
                    visualEffectState = VisualEffectState.create(
                        "armor_equip_flash",
                        1.0,
                        ARMOR_EQUIP_FLASH_DURATION_MS,
                        System.currentTimeMillis()
                    );
                }
            }
            case "dropped" -> {
                Location from = parseLocation(readRequiredObject(payload, "from"));
                WorldPos worldPos = parseWorldPos(payload);
                InventoryItem droppedItem = parseInventoryItem(readRequiredObject(payload, "item"));
                if (from == null || worldPos == null || droppedItem == null || droppedItem.instanceId() != instanceId) {
                    return ServerDataDispatch.noOp(envelope.type(),
                        "Ignoring inventory_event 'dropped' payload: invalid from/world_pos/item payload");
                }
                if (from instanceof ContainerLoc loc) {
                    DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
                        instanceId,
                        loc.containerId(),
                        loc.row(),
                        loc.col(),
                        worldPos.x(),
                        worldPos.y(),
                        worldPos.z(),
                        droppedItem
                    ));
                } else if (from instanceof EquipLoc loc) {
                    DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
                        instanceId,
                        loc.slot().name().toLowerCase(java.util.Locale.ROOT),
                        0,
                        0,
                        worldPos.x(),
                        worldPos.y(),
                        worldPos.z(),
                        droppedItem
                    ));
                } else if (from instanceof HotbarLoc loc) {
                    DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
                        instanceId,
                        "hotbar",
                        0,
                        loc.index(),
                        worldPos.x(),
                        worldPos.y(),
                        worldPos.z(),
                        droppedItem
                    ));
                }
                next = applyDropped(current, instanceId);
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

                // If an equipped armor profile breaks (durability hits 0), surface a short toast.
                InventoryItem existing = findItem(current, instanceId);
                boolean equippedArmor = isEquippedArmor(current, existing, instanceId);
                if (existing != null
                    && existing.durability() > 0.0
                    && durability <= 0.0
                    && equippedArmor) {
                    EquipSlotType slot = armorSlotForInstance(current, instanceId);
                    String label = slot == null ? "护甲" : slot.displayName();
                    alertToast = new ServerDataDispatch.ToastSpec(
                        label + "破损",
                        ARMOR_BROKEN_TOAST_COLOR,
                        ARMOR_BROKEN_TOAST_DURATION_MS
                    );
                    visualEffectState = VisualEffectState.create(
                        "armor_break_flash",
                        1.0,
                        ARMOR_BREAK_FLASH_DURATION_MS,
                        System.currentTimeMillis()
                    );
                    ArmorBreakParticles.spawnLocalShards();
                } else if (existing != null
                    && existing.durability() >= ARMOR_LOW_DURABILITY_THRESHOLD
                    && durability > 0.0
                    && durability < ARMOR_LOW_DURABILITY_THRESHOLD
                    && equippedArmor) {
                    alertToast = new ServerDataDispatch.ToastSpec(
                        "甲胄将破",
                        ARMOR_WARNING_TOAST_COLOR,
                        ARMOR_WARNING_TOAST_DURATION_MS
                    );
                    visualEffectState = VisualEffectState.create(
                        "armor_low_durability_flash",
                        1.0,
                        ARMOR_LOW_DURABILITY_FLASH_DURATION_MS,
                        System.currentTimeMillis()
                    );
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
        if (alertToast != null || !visualEffectState.isEmpty()) {
            return ServerDataDispatch.handledWithEventAlert(
                envelope.type(),
                alertToast,
                visualEffectState,
                "Applied inventory_event '" + kind + "' (instance_id " + instanceId
                    + ", revision " + revision + ") with armor visual cue"
            );
        }
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

    private static InventoryModel applyDropped(InventoryModel current, long instanceId) {
        InventoryItem item = findItem(current, instanceId);
        if (item == null) return null;
        return rebuildWith(current, instanceId, null, null, null);
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

    private static boolean isEquippedArmor(InventoryModel model, InventoryItem item, long instanceId) {
        if (item == null || !ArmorProfileStore.isArmor(item.itemId())) {
            return false;
        }
        EquipSlotType slot = armorSlotForInstance(model, instanceId);
        return slot == EquipSlotType.HEAD
            || slot == EquipSlotType.CHEST
            || slot == EquipSlotType.LEGS
            || slot == EquipSlotType.FEET;
    }

    private static boolean isArmorEquipMove(InventoryItem item, Location to) {
        if (!(to instanceof EquipLoc loc) || item.durability() <= 0.0) {
            return false;
        }
        EquipSlotType slot = loc.slot();
        if (slot != EquipSlotType.HEAD
            && slot != EquipSlotType.CHEST
            && slot != EquipSlotType.LEGS
            && slot != EquipSlotType.FEET) {
            return false;
        }
        ArmorTintRegistry.ArmorItemSpec mundane = ArmorTintRegistry.item(item.itemId());
        if (mundane != null) {
            return fromEquipmentSlot(mundane.slot()) == slot;
        }
        return ArmorProfileStore.isArmor(item.itemId()) && ArmorProfileStore.equipSlotForItemId(item.itemId()) == slot;
    }

    private static EquipSlotType fromEquipmentSlot(EquipmentSlot slot) {
        return switch (slot) {
            case HEAD -> EquipSlotType.HEAD;
            case CHEST -> EquipSlotType.CHEST;
            case LEGS -> EquipSlotType.LEGS;
            case FEET -> EquipSlotType.FEET;
            default -> null;
        };
    }

    private static EquipSlotType armorSlotForInstance(InventoryModel model, long instanceId) {
        for (Map.Entry<EquipSlotType, InventoryItem> e : model.equipped().entrySet()) {
            InventoryItem item = e.getValue();
            if (item != null && item.instanceId() == instanceId) {
                return e.getKey();
            }
        }
        return null;
    }

    private static InventoryItem withStack(InventoryItem item, int stackCount) {
        return InventoryItem.createFullWithVisualMeta(
            item.instanceId(), item.itemId(), item.displayName(),
            item.gridWidth(), item.gridHeight(), item.weight(),
            item.rarity(), item.description(),
            stackCount, item.spiritQuality(), item.durability(),
            item.charges(),
            item.scrollKind(),
            item.scrollSkillId(),
            item.scrollXpGrant(),
            item.forgeQuality(),
            item.forgeColor(),
            item.forgeSideEffects(),
            item.forgeAchievedTier(),
            item.alchemyLines()
        );
    }

    private static InventoryItem withDurability(InventoryItem item, double durability) {
        return InventoryItem.createFullWithVisualMeta(
            item.instanceId(), item.itemId(), item.displayName(),
            item.gridWidth(), item.gridHeight(), item.weight(),
            item.rarity(), item.description(),
            item.stackCount(), item.spiritQuality(), durability,
            item.charges(),
            item.scrollKind(),
            item.scrollSkillId(),
            item.scrollXpGrant(),
            item.forgeQuality(),
            item.forgeColor(),
            item.forgeSideEffects(),
            item.forgeAchievedTier(),
            item.alchemyLines()
        );
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
        JsonElement chargesElement = itemObject.get("charges");
        boolean hasChargesField = chargesElement != null && !chargesElement.isJsonNull();
        Integer charges = readOptionalInt(itemObject, "charges");

        if (instanceId == null || itemId == null || displayName == null
            || gridWidth == null || gridHeight == null || weight == null
            || rarity == null || description == null || stackCount == null
            || spiritQuality == null || durability == null
            || gridWidth < 1 || gridHeight < 1 || weight < 0.0 || stackCount < 1
            || spiritQuality < 0.0 || spiritQuality > 1.0
            || durability < 0.0 || durability > 1.0
            || (hasChargesField && charges == null)
            || (charges != null && (charges < 0 || charges > 5))) {
            return null;
        }

        return InventoryItem.createFullWithVisualMeta(
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
            durability,
            charges,
            "",
            "",
            0,
            null,
            "",
            java.util.List.of(),
            null,
            java.util.List.of()
        );
    }

    /**
     * Reads the dropped-event world position as three flattened sibling fields
     * {@code world_pos_x/world_pos_y/world_pos_z}, matching how {@code InventoryEventDropped}
     * (proto/bong/envelope.proto) lays a Rust {@code [f64;3]} out on the wire — same pattern as
     * {@code ContainerInteractionHandler#readFlatVec3} / {@code ExtractServerDataHandler}.
     * {@code ProtoServerDataBridge#bridgeOneofFlat} flattens the outer {@code InventoryEvent.event}
     * oneof (moved/dropped/stack_changed/durability_changed) to a top-level {@code "kind"} tag but
     * does not reshape these three flat coordinate fields back into a JSON array, so a "dropped"
     * event bridged from the real production proto wire (server {@code --release}) carries
     * {@code world_pos_x/world_pos_y/world_pos_z} as direct sibling fields of the payload, not a
     * {@code "world_pos": [x, y, z]} array. Reading it as an array here (the previous shape) always
     * returned {@code null} on that wire, silently no-op'ing every dropped-item event.
     * Returns {@code null} if any of the three fields is missing or non-numeric.
     */
    private static WorldPos parseWorldPos(JsonObject payload) {
        Double x = readRequiredDouble(payload, "world_pos_x");
        Double y = readRequiredDouble(payload, "world_pos_y");
        Double z = readRequiredDouble(payload, "world_pos_z");
        if (x == null || y == null || z == null) {
            return null;
        }
        return new WorldPos(x, y, z);
    }

    // ─── Location parsing ───────────────────────────────────────────────────

    /**
     * proto-native shape of {@code InventoryLocation.location} (proto/bong/envelope.proto):
     * {@code container}/{@code equip}/{@code hotbar}. {@code ProtoServerDataBridge.bridgeOneofFlat}
     * for {@code inventory_event} only flattens the *outer* {@code InventoryEvent.event} oneof
     * (moved/dropped/stack_changed/durability_changed) into a top-level {@code "kind"} tag — it does
     * not reshape the *nested* {@code InventoryLocation.location} oneof carried by "from"/"to".
     * JsonFormat prints a set oneof case using the proto field name itself as the JSON key
     * (e.g. {@code {"container": {"container_id": ..., "row": ..., "col": ...}}}), not a
     * {@code "kind"} discriminator, so real-proto-sourced "from"/"to" objects never carry a "kind"
     * field at all. {@link #parseLocation} falls back to these raw field names when "kind" is absent.
     */
    private static final String[] PROTO_LOCATION_ONEOF_FIELDS = {"container", "equip", "hotbar"};

    /** {@code EquipSlot} proto enum values print with this prefix (e.g. {@code EQUIP_SLOT_HEAD}). */
    private static final String PROTO_EQUIP_SLOT_PREFIX = "EQUIP_SLOT_";

    private static Location parseLocation(JsonObject obj) {
        if (obj == null) return null;
        String kind = readRequiredString(obj, "kind");
        JsonObject fields = obj;
        if (kind == null) {
            // No "kind" discriminator — try the raw proto InventoryLocation.location oneof shape.
            for (String candidate : PROTO_LOCATION_ONEOF_FIELDS) {
                JsonElement nested = obj.get(candidate);
                if (nested != null && nested.isJsonObject()) {
                    kind = candidate;
                    fields = nested.getAsJsonObject();
                    break;
                }
            }
            if (kind == null) return null;
        }
        return switch (kind) {
            case "container" -> {
                String containerId = readRequiredString(fields, "container_id");
                Long row = readRequiredLong(fields, "row");
                Long col = readRequiredLong(fields, "col");
                if (containerId == null || row == null || col == null
                    || row > Integer.MAX_VALUE || col > Integer.MAX_VALUE) {
                    yield null;
                }
                yield new ContainerLoc(containerId, row.intValue(), col.intValue());
            }
            case "equip" -> {
                String slotName = readRequiredString(fields, "slot");
                if (slotName == null) yield null;
                if (slotName.startsWith(PROTO_EQUIP_SLOT_PREFIX)) {
                    // Proto wire shape: full EquipSlot enum value name (e.g. "EQUIP_SLOT_HEAD"),
                    // not the legacy lowercase wire name ("head") EQUIP_SLOT_BY_WIRE_NAME expects.
                    slotName = slotName.substring(PROTO_EQUIP_SLOT_PREFIX.length())
                        .toLowerCase(java.util.Locale.ROOT);
                }
                EquipSlotType slot = EQUIP_SLOT_BY_WIRE_NAME.get(slotName);
                yield slot == null ? null : new EquipLoc(slot);
            }
            case "hotbar" -> {
                Long index = readRequiredLong(fields, "index");
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

    private static Integer readRequiredInt(JsonObject object, String fieldName) {
        Long value = readRequiredLong(object, fieldName);
        if (value == null || value > Integer.MAX_VALUE) {
            return null;
        }
        return value.intValue();
    }

    private static Integer readOptionalInt(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull()) {
            return null;
        }
        return readRequiredInt(object, fieldName);
    }
}
