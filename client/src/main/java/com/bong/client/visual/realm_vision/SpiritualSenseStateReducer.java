package com.bong.client.visual.realm_vision;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.ArrayList;
import java.util.List;

public final class SpiritualSenseStateReducer {
    private SpiritualSenseStateReducer() {
    }

    public static PerceptionEdgeState apply(JsonObject payload) {
        if (payload == null) {
            return PerceptionEdgeState.empty();
        }
        long generation = Math.max(0L, (long) readDouble(payload, "generation"));
        JsonElement entriesElement = payload.get("entries");
        if (entriesElement == null || !entriesElement.isJsonArray()) {
            return new PerceptionEdgeState(List.of(), generation);
        }

        List<PerceptionEdgeState.SenseEntry> entries = new ArrayList<>();
        JsonArray array = entriesElement.getAsJsonArray();
        for (JsonElement element : array) {
            if (element == null || !element.isJsonObject()) {
                continue;
            }
            JsonObject entry = element.getAsJsonObject();
            entries.add(new PerceptionEdgeState.SenseEntry(
                SenseKind.fromWire(readString(entry, "kind")),
                readDouble(entry, "x"),
                readDouble(entry, "y"),
                readDouble(entry, "z"),
                readDouble(entry, "intensity")
            ));
        }
        return new PerceptionEdgeState(entries, generation);
    }

    static String readString(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        return primitive != null && primitive.isString() ? primitive.getAsString() : null;
    }

    static double readDouble(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isNumber()) {
            return 0.0;
        }
        double value = primitive.getAsDouble();
        return Double.isFinite(value) ? value : 0.0;
    }

    private static JsonPrimitive readPrimitive(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }
        return element.getAsJsonPrimitive();
    }
}
