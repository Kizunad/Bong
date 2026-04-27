package com.bong.client.network;

import com.bong.client.combat.SkillBarConfig;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

/** Handles full 1-9 skillbar_config snapshots from the server. */
public final class SkillBarConfigHandler implements ServerDataHandler {
    private static final int SLOT_COUNT = SkillBarConfig.SLOT_COUNT;

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        JsonArray slotsArr = readArray(payload, "slots");
        JsonArray cdArr = readArray(payload, "cooldown_until_ms");
        if (slotsArr == null || cdArr == null) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring skillbar_config payload: slots / cooldown_until_ms missing");
        }
        if (slotsArr.size() != SLOT_COUNT || cdArr.size() != SLOT_COUNT) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring skillbar_config payload: array length mismatch (expected " + SLOT_COUNT + ")");
        }

        SkillBarEntry[] entries = new SkillBarEntry[SLOT_COUNT];
        for (int i = 0; i < SLOT_COUNT; i++) {
            JsonElement el = slotsArr.get(i);
            if (el == null || el.isJsonNull()) continue;
            if (!el.isJsonObject()) {
                return ServerDataDispatch.noOp(envelope.type(),
                    "Ignoring skillbar_config payload: slot " + i + " not an object");
            }
            SkillBarEntry entry = parseEntry(el.getAsJsonObject());
            if (entry == null) {
                return ServerDataDispatch.noOp(envelope.type(),
                    "Ignoring skillbar_config payload: invalid slot " + i + " entry");
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
            if (v >= 0) cooldowns[i] = v;
        }

        SkillBarStore.replace(SkillBarConfig.of(entries, cooldowns));
        return ServerDataDispatch.handled(envelope.type(),
            "Applied skillbar_config (" + countNonNull(entries) + " bound slots)");
    }

    private static SkillBarEntry parseEntry(JsonObject obj) {
        String kind = readString(obj, "kind");
        if (kind == null) return null;
        String displayName = readString(obj, "display_name");
        int castDuration = (int) Math.min(readLong(obj, "cast_duration_ms", 0L), Integer.MAX_VALUE);
        int cooldown = (int) Math.min(readLong(obj, "cooldown_ms", 0L), Integer.MAX_VALUE);
        String icon = readString(obj, "icon_texture");
        return switch (kind) {
            case "item" -> {
                String templateId = readString(obj, "template_id");
                yield templateId == null || templateId.isEmpty()
                    ? null
                    : SkillBarEntry.item(templateId, displayName, castDuration, cooldown, icon);
            }
            case "skill" -> {
                String skillId = readString(obj, "skill_id");
                yield skillId == null || skillId.isEmpty()
                    ? null
                    : SkillBarEntry.skill(skillId, displayName, castDuration, cooldown, icon);
            }
            default -> null;
        };
    }

    private static int countNonNull(SkillBarEntry[] entries) {
        int n = 0;
        for (SkillBarEntry entry : entries) if (entry != null) n++;
        return n;
    }

    static JsonArray readArray(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        return (el != null && !el.isJsonNull() && el.isJsonArray()) ? el.getAsJsonArray() : null;
    }

    static String readString(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        JsonPrimitive p = el.getAsJsonPrimitive();
        return p.isString() ? p.getAsString() : null;
    }

    static long readLong(JsonObject obj, String field, long fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isNumber()) return fallback;
        long v = p.getAsLong();
        return v < 0 ? fallback : v;
    }
}
