package com.bong.client.npc;

import com.bong.client.network.ServerDataEnvelope;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

public final class NpcMetadataHandler {
    public static final String CHANNEL_NAMESPACE = "bong";
    public static final String CHANNEL_PATH = "npc_metadata";
    public static final int VERSION = 1;

    private NpcMetadataHandler() {
    }

    public static boolean handle(String jsonPayload, int payloadSizeBytes) {
        if (jsonPayload == null || payloadSizeBytes < 0 || payloadSizeBytes > ServerDataEnvelope.MAX_PAYLOAD_BYTES) {
            return false;
        }
        JsonObject root;
        try {
            root = JsonParser.parseString(jsonPayload).getAsJsonObject();
        } catch (RuntimeException exception) {
            return false;
        }
        if (intField(root, "v", -1) != VERSION) {
            return false;
        }
        if (!"npc_metadata".equals(stringField(root, "type", ""))) {
            return false;
        }
        int entityId = intField(root, "entity_id", Integer.MIN_VALUE);
        if (entityId == Integer.MIN_VALUE || entityId < 0) {
            return false;
        }

        NpcMetadataStore.upsert(new NpcMetadata(
            entityId,
            stringField(root, "archetype", "unknown"),
            stringField(root, "realm", "未知"),
            nullableString(root, "faction_name"),
            nullableString(root, "faction_rank"),
            intField(root, "reputation_to_player", 0),
            stringField(root, "display_name", ""),
            stringField(root, "age_band", "正值壮年"),
            stringField(root, "greeting_text", "对方沉默地看着你。"),
            nullableString(root, "qi_hint"),
            doubleField(root, "hp_ratio", 1.0),
            doubleField(root, "qi_ratio", 0.0)
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

    private static String stringField(JsonObject root, String fieldName, String fallback) {
        String value = nullableString(root, fieldName);
        return value == null ? fallback : value;
    }

    private static String nullableString(JsonObject root, String fieldName) {
        if (!root.has(fieldName) || root.get(fieldName).isJsonNull()) {
            return null;
        }
        try {
            return root.get(fieldName).getAsString();
        } catch (RuntimeException exception) {
            return null;
        }
    }

    private static double doubleField(JsonObject root, String fieldName, double fallback) {
        if (!root.has(fieldName) || root.get(fieldName).isJsonNull()) {
            return fallback;
        }
        try {
            double value = root.get(fieldName).getAsDouble();
            return Double.isFinite(value) ? value : fallback;
        } catch (RuntimeException exception) {
            return fallback;
        }
    }
}
