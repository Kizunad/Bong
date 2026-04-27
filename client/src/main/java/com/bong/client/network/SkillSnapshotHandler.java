package com.bong.client.network;

import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillSetSnapshot;
import com.bong.client.skill.SkillSetStore;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.Map;
import java.util.Set;
import java.util.regex.Pattern;

public final class SkillSnapshotHandler implements ServerDataHandler {
    private static final Pattern INTEGER_TOKEN_PATTERN = Pattern.compile("-?(0|[1-9]\\d*)");

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        JsonObject skillsObject = readObject(payload, "skills");
        if (skillsObject == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring skill_snapshot payload: missing or invalid skills object"
            );
        }

        Map<SkillId, SkillSetSnapshot.Entry> next = new LinkedHashMap<>();
        for (SkillId skillId : SkillId.values()) {
            JsonObject skillObject = readObject(skillsObject, skillId.wireId());
            if (skillObject == null) {
                return ServerDataDispatch.noOp(
                    envelope.type(),
                    "Ignoring skill_snapshot payload: missing skill entry for " + skillId.wireId()
                );
            }
            SkillSetSnapshot.Entry entry = parseEntry(skillObject);
            if (entry == null) {
                return ServerDataDispatch.noOp(
                    envelope.type(),
                    "Ignoring skill_snapshot payload: invalid entry for " + skillId.wireId()
                );
            }
            next.put(skillId, entry);
        }

        Set<String> consumedScrolls = parseConsumedScrolls(payload.getAsJsonArray("consumed_scrolls"));

        SkillSetStore.replace(SkillSetSnapshot.of(next, consumedScrolls));
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied skill_snapshot to SkillSetStore"
        );
    }

    private static Set<String> parseConsumedScrolls(JsonArray array) {
        if (array == null) return Set.of();
        LinkedHashSet<String> out = new LinkedHashSet<>();
        for (JsonElement element : array) {
            if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) continue;
            JsonPrimitive primitive = element.getAsJsonPrimitive();
            if (!primitive.isString()) continue;
            String scrollId = primitive.getAsString();
            if (scrollId != null && !scrollId.isBlank()) {
                out.add(scrollId.trim());
            }
        }
        return Set.copyOf(out);
    }

    private static SkillSetSnapshot.Entry parseEntry(JsonObject object) {
        Integer lv = readInt(object, "lv");
        Long xp = readLong(object, "xp");
        Long xpToNext = readLong(object, "xp_to_next");
        Long totalXp = readLong(object, "total_xp");
        Integer cap = readInt(object, "cap");
        Long recentGainXp = readLong(object, "recent_gain_xp");
        if (lv == null || xp == null || xpToNext == null || totalXp == null || cap == null || recentGainXp == null) {
            return null;
        }
        return new SkillSetSnapshot.Entry(
            lv,
            xp,
            xpToNext,
            totalXp,
            cap,
            recentGainXp,
            0L
        );
    }

    private static JsonObject readObject(JsonObject object, String fieldName) {
        JsonElement element = object == null ? null : object.get(fieldName);
        return element != null && element.isJsonObject() ? element.getAsJsonObject() : null;
    }

    private static JsonPrimitive readPrimitive(JsonObject object, String fieldName) {
        JsonElement element = object == null ? null : object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }
        return element.getAsJsonPrimitive();
    }

    private static Integer readInt(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isNumber()) return null;
        String raw = primitive.getAsString();
        if (!INTEGER_TOKEN_PATTERN.matcher(raw).matches()) return null;
        try { return Integer.parseInt(raw); } catch (NumberFormatException e) { return null; }
    }

    private static Long readLong(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isNumber()) return null;
        String raw = primitive.getAsString();
        if (!INTEGER_TOKEN_PATTERN.matcher(raw).matches()) return null;
        try { return Long.parseLong(raw); } catch (NumberFormatException e) { return null; }
    }
}
