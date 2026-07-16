package com.bong.client.dandao;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.JsonPrimitive;

import java.util.LinkedHashMap;
import java.util.Map;

/**
 * plan-race-system-v1 P0 review r3 -- parses the
 * {@code assets/bong/body_plans/humanoid_mutation_slots.json} shared resource into a
 * {@link MutationSlotLayout}. Mirrors {@code ZoneAtmosphereProfileParser}'s
 * try/catch + {@code ParseResult} shape (established precedent for classpath JSON
 * resources in this client, see {@code com.bong.client.atmosphere}).
 */
public final class MutationSlotLayoutParser {
    private MutationSlotLayoutParser() {}

    public static ParseResult parse(String json) {
        try {
            JsonObject root = JsonParser.parseString(json).getAsJsonObject();
            String bodyPlanId = readString(root, "body_plan_id", "humanoid");
            JsonElement slotsElement = root.get("slots");
            if (slotsElement == null || !slotsElement.isJsonObject()) {
                throw new IllegalArgumentException("`slots` must be a JSON object");
            }
            JsonObject slotsObj = slotsElement.getAsJsonObject();

            Map<String, MutationSlotLayout.SlotEntry> slots = new LinkedHashMap<>();
            for (Map.Entry<String, JsonElement> entry : slotsObj.entrySet()) {
                if (!entry.getValue().isJsonObject()) {
                    throw new IllegalArgumentException("slot '" + entry.getKey() + "' must be an object");
                }
                JsonObject slotObj = entry.getValue().getAsJsonObject();
                String partId = readString(slotObj, "part_id", null);
                if (partId == null || partId.isBlank()) {
                    throw new IllegalArgumentException("slot '" + entry.getKey() + "' missing part_id");
                }
                MutationSlotLayout.Anchor anchor = readAnchor(slotObj, entry.getKey());
                slots.put(entry.getKey(), new MutationSlotLayout.SlotEntry(partId, anchor));
            }

            return ParseResult.success(new MutationSlotLayout(bodyPlanId, slots));
        } catch (RuntimeException ex) {
            return ParseResult.error(ex.getMessage() == null ? ex.getClass().getSimpleName() : ex.getMessage());
        }
    }

    private static MutationSlotLayout.Anchor readAnchor(JsonObject slotObj, String slotKey) {
        JsonElement anchorElement = slotObj.get("anchor");
        if (anchorElement == null || anchorElement.isJsonNull()) {
            return MutationSlotLayout.Anchor.IDENTITY;
        }
        if (!anchorElement.isJsonObject()) {
            throw new IllegalArgumentException("slot '" + slotKey + "' anchor must be an object");
        }
        JsonObject anchorObj = anchorElement.getAsJsonObject();
        float[] offset = readOffset(anchorObj, slotKey);
        float scale = (float) readDouble(anchorObj, "scale", 1.0);
        return new MutationSlotLayout.Anchor(offset[0], offset[1], offset[2], scale);
    }

    private static float[] readOffset(JsonObject anchorObj, String slotKey) {
        JsonElement offsetElement = anchorObj.get("offset");
        if (offsetElement == null || !offsetElement.isJsonArray()) {
            throw new IllegalArgumentException("slot '" + slotKey + "' anchor.offset must be a [x,y,z] array");
        }
        JsonArray array = offsetElement.getAsJsonArray();
        if (array.size() != 3) {
            throw new IllegalArgumentException("slot '" + slotKey + "' anchor.offset must have exactly 3 elements");
        }
        return new float[] {
            array.get(0).getAsFloat(),
            array.get(1).getAsFloat(),
            array.get(2).getAsFloat()
        };
    }

    private static String readString(JsonObject object, String field, String fallback) {
        JsonElement element = object.get(field);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return fallback;
        }
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        return primitive.isString() ? primitive.getAsString() : fallback;
    }

    private static double readDouble(JsonObject object, String field, double fallback) {
        JsonElement element = object.get(field);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return fallback;
        }
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        return primitive.isNumber() ? primitive.getAsDouble() : fallback;
    }

    public record ParseResult(MutationSlotLayout layout, String error) {
        static ParseResult success(MutationSlotLayout layout) {
            return new ParseResult(layout, "");
        }

        static ParseResult error(String error) {
            return new ParseResult(null, error == null ? "parse error" : error);
        }

        public boolean ok() {
            return layout != null;
        }
    }
}
