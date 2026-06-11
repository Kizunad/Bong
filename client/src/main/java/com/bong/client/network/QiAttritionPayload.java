package com.bong.client.network;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import java.nio.charset.StandardCharsets;

public final class QiAttritionPayload {
    public static final int EXPECTED_VERSION = 1;
    public static final int MAX_PAYLOAD_BYTES = 1024;

    private final long itemEntityId;
    private final double amountLost;
    private final double[] worldPos;

    public QiAttritionPayload(long itemEntityId, double amountLost, double[] worldPos) {
        this.itemEntityId = itemEntityId;
        this.amountLost = amountLost;
        this.worldPos = worldPos == null ? new double[] {0.0, 64.0, 0.0} : worldPos.clone();
    }

    public long itemEntityId() {
        return itemEntityId;
    }

    public double amountLost() {
        return amountLost;
    }

    public double[] worldPos() {
        return worldPos.clone();
    }

    public static String decodeUtf8(byte[] bytes) {
        return new String(bytes, StandardCharsets.UTF_8);
    }

    public static ParseResult parse(String jsonPayload, int payloadSizeBytes) {
        if (jsonPayload == null) {
            return ParseResult.error("Malformed JSON: payload was null");
        }
        if (payloadSizeBytes < 0 || payloadSizeBytes > MAX_PAYLOAD_BYTES) {
            return ParseResult.error("Payload size out of range");
        }

        try {
            JsonElement rootElement = JsonParser.parseString(jsonPayload);
            if (!rootElement.isJsonObject()) {
                return ParseResult.error("Malformed JSON: expected top-level object");
            }
            JsonObject root = rootElement.getAsJsonObject();

            Integer version = readInteger(root, "v");
            if (version == null) {
                return ParseResult.error("Missing version 'v'");
            }
            if (version != EXPECTED_VERSION) {
                return ParseResult.error("Unsupported version: " + version);
            }

            Long itemEntityId = readLong(root, "item_entity_id");
            if (itemEntityId == null || itemEntityId < 0) {
                return ParseResult.error("Invalid or missing 'item_entity_id'");
            }

            Double amountLost = readDouble(root, "amount_lost");
            if (amountLost == null || !Double.isFinite(amountLost) || amountLost <= 0.0) {
                return ParseResult.error("Invalid or missing 'amount_lost'");
            }

            double[] worldPos = readVec3(root.get("world_pos"));
            if (worldPos == null) {
                return ParseResult.error("Invalid or missing 'world_pos'");
            }

            return ParseResult.success(new QiAttritionPayload(itemEntityId, amountLost, worldPos));
        } catch (RuntimeException exception) {
            return ParseResult.error("Malformed JSON: " + exception.getMessage());
        }
    }

    private static Integer readInteger(JsonObject root, String fieldName) {
        JsonElement element = root.get(fieldName);
        if (element == null || !element.isJsonPrimitive() || !element.getAsJsonPrimitive().isNumber()) {
            return null;
        }
        try {
            return element.getAsInt();
        } catch (RuntimeException exception) {
            return null;
        }
    }

    private static Long readLong(JsonObject root, String fieldName) {
        JsonElement element = root.get(fieldName);
        if (element == null || !element.isJsonPrimitive() || !element.getAsJsonPrimitive().isNumber()) {
            return null;
        }
        try {
            return element.getAsLong();
        } catch (RuntimeException exception) {
            return null;
        }
    }

    private static Double readDouble(JsonObject root, String fieldName) {
        JsonElement element = root.get(fieldName);
        if (element == null || !element.isJsonPrimitive() || !element.getAsJsonPrimitive().isNumber()) {
            return null;
        }
        try {
            return element.getAsDouble();
        } catch (RuntimeException exception) {
            return null;
        }
    }

    private static double[] readVec3(JsonElement element) {
        if (element == null || !element.isJsonArray()) {
            return null;
        }
        JsonArray array = element.getAsJsonArray();
        if (array.size() != 3) {
            return null;
        }
        double[] values = new double[3];
        for (int i = 0; i < 3; i++) {
            JsonElement value = array.get(i);
            if (!value.isJsonPrimitive() || !value.getAsJsonPrimitive().isNumber()) {
                return null;
            }
            values[i] = value.getAsDouble();
            if (!Double.isFinite(values[i])) {
                return null;
            }
        }
        return values;
    }

    public record ParseResult(boolean success, QiAttritionPayload payload, String errorMessage) {
        static ParseResult success(QiAttritionPayload payload) {
            return new ParseResult(true, payload, "");
        }

        static ParseResult error(String message) {
            return new ParseResult(false, null, message);
        }
    }
}
