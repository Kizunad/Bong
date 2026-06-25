package com.bong.client.tiandao;

import com.bong.client.network.ServerDataEnvelope;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

public final class TiandaoPresencePayloadHandler {
    private TiandaoPresencePayloadHandler() {
    }

    public static Result handle(String jsonPayload, int payloadSizeBytes) {
        if (jsonPayload == null || payloadSizeBytes < 0 || payloadSizeBytes > ServerDataEnvelope.MAX_PAYLOAD_BYTES) {
            return Result.error("invalid payload size");
        }
        try {
            JsonElement rootElement = JsonParser.parseString(jsonPayload);
            if (!rootElement.isJsonObject()) {
                return Result.error("expected object");
            }
            JsonObject root = rootElement.getAsJsonObject();
            if (intField(root, "v", -1) != 1) {
                return Result.error("unsupported version");
            }
            String type = stringField(root, "type", "");
            if (!"tiandao_presence".equals(type)) {
                return Result.error("unexpected type");
            }
            TiandaoPresenceStore.replace(new TiandaoPresenceState(
                true,
                stringField(root, "response", "none"),
                doubleField(root, "level", 0.0),
                stringField(root, "zone", ""),
                doubleField(root, "zone_spirit_qi", 0.0),
                intField(root, "vignette_rgb", 0),
                doubleField(root, "vignette_alpha", 0.0),
                doubleField(root, "shake_intensity", 0.0),
                doubleField(root, "saturation", 1.0),
                longField(root, "tick", 0L)
            ));
            return Result.ok("tiandao_presence:" + stringField(root, "response", "none"));
        } catch (RuntimeException exception) {
            return Result.error("malformed json: " + exception.getMessage());
        }
    }

    private static String stringField(JsonObject root, String key, String fallback) {
        JsonElement value = root.get(key);
        return value != null && value.isJsonPrimitive() ? value.getAsString() : fallback;
    }

    private static int intField(JsonObject root, String key, int fallback) {
        JsonElement value = root.get(key);
        try {
            return value != null && value.isJsonPrimitive() ? value.getAsInt() : fallback;
        } catch (RuntimeException ignored) {
            return fallback;
        }
    }

    private static long longField(JsonObject root, String key, long fallback) {
        JsonElement value = root.get(key);
        try {
            return value != null && value.isJsonPrimitive() ? value.getAsLong() : fallback;
        } catch (RuntimeException ignored) {
            return fallback;
        }
    }

    private static double doubleField(JsonObject root, String key, double fallback) {
        JsonElement value = root.get(key);
        try {
            return value != null && value.isJsonPrimitive() ? value.getAsDouble() : fallback;
        } catch (RuntimeException ignored) {
            return fallback;
        }
    }

    public record Result(boolean handled, String logMessage) {
        static Result ok(String message) {
            return new Result(true, message);
        }

        static Result error(String message) {
            return new Result(false, message);
        }
    }
}
