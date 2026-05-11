package com.bong.client.tsy;

import com.bong.client.network.ServerDataEnvelope;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

public final class TsyDeathVfxHandler {
    public static final String CHANNEL_NAMESPACE = "bong";
    public static final String CHANNEL_PATH = "tsy_death_vfx";

    private TsyDeathVfxHandler() {
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
        if (intField(root, "v", -1) != 1 || !"tsy_death_vfx".equals(stringField(root, "type", ""))) {
            return false;
        }
        TsyDeathVfxStore.trigger(nowMillis);
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
