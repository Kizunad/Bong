package com.bong.client.tsy;

import com.bong.client.network.ServerDataEnvelope;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

public final class TsyBossHealthHandler {
    public static final String CHANNEL_NAMESPACE = "bong";
    public static final String CHANNEL_PATH = "tsy_boss_health";

    private TsyBossHealthHandler() {
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
        if (intField(root, "v", -1) != 1 || !"tsy_boss_health".equals(stringField(root, "type", ""))) {
            return false;
        }
        TsyBossHealthStore.replace(new TsyBossHealthState(
            boolField(root, "active", true),
            stringField(root, "boss_name", "秘境守灵"),
            stringField(root, "realm", "未知"),
            doubleField(root, "health_ratio", 0.0),
            intField(root, "phase", 1),
            intField(root, "max_phase", 1),
            nowMillis
        ));
        return true;
    }

    private static boolean boolField(JsonObject root, String fieldName, boolean fallback) {
        if (!root.has(fieldName) || root.get(fieldName).isJsonNull()) {
            return fallback;
        }
        try {
            return root.get(fieldName).getAsBoolean();
        } catch (RuntimeException exception) {
            return fallback;
        }
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
        if (!root.has(fieldName) || root.get(fieldName).isJsonNull()) {
            return fallback;
        }
        try {
            String value = root.get(fieldName).getAsString();
            return value == null || value.isBlank() ? fallback : value.trim();
        } catch (RuntimeException exception) {
            return fallback;
        }
    }
}
