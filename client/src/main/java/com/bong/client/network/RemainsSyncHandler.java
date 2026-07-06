package com.bong.client.network;

import com.bong.client.inventory.state.RemainsStore;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import java.util.ArrayList;
import java.util.List;

/**
 * plan-remains-suite P0 — 世界内遗骸容器同步（照 {@link DroppedLootSyncHandler} 的形状）。
 *
 * <p>Wire 形状 = proto。生产 --release 走 protobuf：`RemainsEntry` 的 [f64;3] 世界坐标在
 * proto 侧拆成 flat {@code world_pos_x/y/z} 三字段，经 {@link ProtoServerDataBridge}
 * （preservingProtoFieldNames）展平后即为这三个 key——与 dropped_loot_sync 同款，参见
 * {@link DroppedLootSyncHandler} 里那段"曾经读 world_pos 数组导致 proto 路径全丢"的教训，
 * 这里直接照抄正确形状，不重踩那个坑。</p>
 */
public final class RemainsSyncHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonArray remains = readRequiredArray(envelope.payload(), "remains");
        if (remains == null) {
            return ServerDataDispatch.noOp(envelope.type(), "Ignoring remains_sync payload: missing remains array");
        }

        List<RemainsStore.Entry> entries = new ArrayList<>();
        for (JsonElement element : remains) {
            if (!element.isJsonObject()) {
                return ServerDataDispatch.noOp(envelope.type(), "Ignoring remains_sync payload: malformed remains entry");
            }
            RemainsStore.Entry entry = parseEntry(element.getAsJsonObject());
            if (entry == null) {
                return ServerDataDispatch.noOp(envelope.type(), "Ignoring remains_sync payload: invalid remains entry");
            }
            entries.add(entry);
        }

        RemainsStore.replaceAll(entries);
        return ServerDataDispatch.handled(envelope.type(), "Applied remains_sync with " + entries.size() + " entries");
    }

    private static RemainsStore.Entry parseEntry(JsonObject object) {
        String remainsId = readRequiredString(object, "remains_id");
        Double x = readRequiredDouble(object, "world_pos_x");
        Double y = readRequiredDouble(object, "world_pos_y");
        Double z = readRequiredDouble(object, "world_pos_z");
        String dimension = readRequiredString(object, "dimension");
        String displayName = readRequiredString(object, "display_name");
        Integer itemCount = readRequiredNonNegativeInt(object, "item_count");
        Long boneCoins = readRequiredNonNegativeLong(object, "bone_coins");
        if (remainsId == null || x == null || y == null || z == null || dimension == null
            || displayName == null || itemCount == null || boneCoins == null) {
            return null;
        }
        return new RemainsStore.Entry(remainsId, x, y, z, dimension, displayName, itemCount, boneCoins);
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

    private static Long readRequiredLong(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) return null;
        try {
            return element.getAsLong();
        } catch (RuntimeException e) {
            return null;
        }
    }

    private static Long readRequiredNonNegativeLong(JsonObject object, String fieldName) {
        Long value = readRequiredLong(object, fieldName);
        return value == null || value < 0L ? null : value;
    }

    private static Integer readRequiredNonNegativeInt(JsonObject object, String fieldName) {
        Long value = readRequiredLong(object, fieldName);
        return value == null || value < 0L || value > Integer.MAX_VALUE ? null : value.intValue();
    }

    private static Double readRequiredDouble(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) return null;
        try {
            return element.getAsDouble();
        } catch (RuntimeException e) {
            return null;
        }
    }
}
