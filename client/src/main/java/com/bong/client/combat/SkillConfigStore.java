package com.bong.client.combat;

import com.google.gson.JsonObject;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

/** Client-side mirror of per-technique SkillConfig snapshots. */
public final class SkillConfigStore {
    private static volatile Map<String, JsonObject> snapshot = Collections.emptyMap();

    private SkillConfigStore() {
    }

    public static Map<String, JsonObject> snapshot() {
        if (snapshot.isEmpty()) return Collections.emptyMap();
        Map<String, JsonObject> copy = new LinkedHashMap<>();
        for (Map.Entry<String, JsonObject> entry : snapshot.entrySet()) {
            copy.put(entry.getKey(), entry.getValue().deepCopy());
        }
        return Collections.unmodifiableMap(copy);
    }

    public static JsonObject configFor(String skillId) {
        if (skillId == null || skillId.isBlank()) return null;
        JsonObject config = snapshot.get(skillId);
        return config == null ? null : config.deepCopy();
    }

    public static void replace(Map<String, JsonObject> configs) {
        if (configs == null || configs.isEmpty()) {
            snapshot = Collections.emptyMap();
            return;
        }
        Map<String, JsonObject> next = new LinkedHashMap<>();
        for (Map.Entry<String, JsonObject> entry : configs.entrySet()) {
            if (entry.getKey() == null || entry.getKey().isBlank() || entry.getValue() == null) continue;
            next.put(entry.getKey(), entry.getValue().deepCopy());
        }
        snapshot = next.isEmpty() ? Collections.emptyMap() : Collections.unmodifiableMap(next);
    }

    public static void updateLocal(String skillId, JsonObject config) {
        if (skillId == null || skillId.isBlank()) return;
        Map<String, JsonObject> next = new LinkedHashMap<>(snapshot);
        if (config == null || config.size() == 0) {
            next.remove(skillId);
        } else {
            next.put(skillId, config.deepCopy());
        }
        snapshot = next.isEmpty() ? Collections.emptyMap() : Collections.unmodifiableMap(next);
    }

    public static void resetForTests() {
        snapshot = Collections.emptyMap();
    }
}
