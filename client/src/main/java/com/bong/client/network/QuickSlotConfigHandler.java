package com.bong.client.network;

import com.bong.client.combat.QuickSlotConfig;
import com.bong.client.combat.QuickSlotEntry;
import com.bong.client.combat.QuickUseSlotStore;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

/**
 * plan-HUD-v1 §10.4 / §11.4 quickslot_config 客户端 handler。
 * server 在 {@code QuickSlotBindings} 变化（绑定 / cast 完成 / 中断写 cooldown）
 * 时推完整 config，本 handler 整体替换 {@link QuickUseSlotStore}。
 */
public final class QuickSlotConfigHandler implements ServerDataHandler {
    private static final int SLOT_COUNT = QuickSlotConfig.SLOT_COUNT;

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        JsonArray slotsArr = readArray(payload, "slots");
        JsonArray cdArr = readArray(payload, "cooldown_until_ms");
        if (slotsArr == null || cdArr == null) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring quickslot_config payload: slots / cooldown_until_ms missing");
        }
        if (slotsArr.size() != SLOT_COUNT || cdArr.size() != SLOT_COUNT) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring quickslot_config payload: array length mismatch (expected "
                    + SLOT_COUNT + ")");
        }

        QuickSlotEntry[] entries = new QuickSlotEntry[SLOT_COUNT];
        for (int i = 0; i < SLOT_COUNT; i++) {
            JsonElement el = slotsArr.get(i);
            if (el == null || el.isJsonNull()) continue;
            if (!el.isJsonObject()) {
                return ServerDataDispatch.noOp(envelope.type(),
                    "Ignoring quickslot_config payload: slot " + i + " not an object");
            }
            QuickSlotEntry entry = parseEntry(el.getAsJsonObject());
            if (entry == null) {
                return ServerDataDispatch.noOp(envelope.type(),
                    "Ignoring quickslot_config payload: invalid slot " + i + " entry");
            }
            entries[i] = entry;
        }

        long[] cooldowns = new long[SLOT_COUNT];
        for (int i = 0; i < SLOT_COUNT; i++) {
            JsonElement el = cdArr.get(i);
            if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) continue;
            JsonPrimitive p = el.getAsJsonPrimitive();
            if (!p.isNumber()) continue;
            long v = p.getAsLong();
            if (v < 0) continue;
            cooldowns[i] = v;
        }

        QuickUseSlotStore.replace(QuickSlotConfig.of(entries, cooldowns));
        return ServerDataDispatch.handled(envelope.type(),
            "Applied quickslot_config (" + countNonNull(entries) + " bound slots)");
    }

    private static QuickSlotEntry parseEntry(JsonObject obj) {
        String itemId = readString(obj, "item_id");
        if (itemId == null || itemId.isEmpty()) return null;
        String displayName = readString(obj, "display_name");
        long castDuration = readLong(obj, "cast_duration_ms", 0L);
        long cooldown = readLong(obj, "cooldown_ms", 0L);
        String icon = readString(obj, "icon_texture");
        return new QuickSlotEntry(
            itemId,
            displayName == null ? "" : displayName,
            (int) Math.min(castDuration, Integer.MAX_VALUE),
            (int) Math.min(cooldown, Integer.MAX_VALUE),
            icon == null ? "" : icon
        );
    }

    private static int countNonNull(QuickSlotEntry[] entries) {
        int n = 0;
        for (QuickSlotEntry e : entries) if (e != null) n++;
        return n;
    }

    private static JsonArray readArray(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        return (el != null && !el.isJsonNull() && el.isJsonArray()) ? el.getAsJsonArray() : null;
    }

    private static String readString(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        JsonPrimitive p = el.getAsJsonPrimitive();
        return p.isString() ? p.getAsString() : null;
    }

    private static long readLong(JsonObject obj, String field, long fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isNumber()) return fallback;
        long v = p.getAsLong();
        return v < 0 ? fallback : v;
    }
}
