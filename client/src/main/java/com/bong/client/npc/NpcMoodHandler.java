package com.bong.client.npc;

import com.bong.client.network.ServerDataEnvelope;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

public final class NpcMoodHandler {
    public static final String CHANNEL_NAMESPACE = "bong";
    public static final String CHANNEL_PATH = "npc_mood";
    public static final int VERSION = 1;

    private NpcMoodHandler() {
    }

    public static boolean handle(String jsonPayload, int payloadSizeBytes, long nowMillis) {
        if (jsonPayload == null || payloadSizeBytes < 0 || payloadSizeBytes > ServerDataEnvelope.MAX_PAYLOAD_BYTES) {
            return false;
        }
        JsonObject root;
        try {
            root = JsonParser.parseString(jsonPayload).getAsJsonObject();
        } catch (RuntimeException exception) {
            return false;
        }
        if (intField(root, "v", -1) != VERSION || !"npc_mood".equals(stringField(root, "type", ""))) {
            return false;
        }
        int entityId = intField(root, "entity_id", Integer.MIN_VALUE);
        if (entityId < 0) {
            return false;
        }
        NpcMoodStore.upsert(new NpcMoodState(
            entityId,
            stringField(root, "mood", "neutral"),
            doubleField(root, "threat_level", 0.0),
            nullableString(root, "qi_level_hint"),
            nullableString(root, "inner_monologue"),
            nowMillis
        ));
        return true;
    }

    private static int intField(JsonObject root, String fieldName, int fallback) {
        if (!root.has(fieldName) || root.get(fieldName).isJsonNull()) {
            return fallback;
        }
        try {
            return root.get(fieldName).getAsInt();
        } catch (RuntimeException exception) {
            return fallback;
        }
    }

    private static double doubleField(JsonObject root, String fieldName, double fallback) {
        if (!root.has(fieldName) || root.get(fieldName).isJsonNull()) {
            return fallback;
        }
        try {
            return root.get(fieldName).getAsDouble();
        } catch (RuntimeException exception) {
            return fallback;
        }
    }

    private static String stringField(JsonObject root, String fieldName, String fallback) {
        String value = nullableString(root, fieldName);
        return value == null ? fallback : value;
    }

    private static String nullableString(JsonObject root, String fieldName) {
        if (!root.has(fieldName) || root.get(fieldName).isJsonNull()) {
            return null;
        }
        try {
            String value = root.get(fieldName).getAsString();
            return value == null || value.isBlank() ? null : value.trim();
        } catch (RuntimeException exception) {
            return null;
        }
    }
}
