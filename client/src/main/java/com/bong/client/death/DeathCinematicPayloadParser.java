package com.bong.client.death;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.ArrayList;
import java.util.List;

public final class DeathCinematicPayloadParser {
    private DeathCinematicPayloadParser() {}

    public static DeathCinematicState parse(JsonObject obj) {
        return parse(obj, System.currentTimeMillis());
    }

    static DeathCinematicState parse(JsonObject obj, long receivedAtMillis) {
        if (obj == null) {
            return DeathCinematicState.INACTIVE;
        }
        JsonObject rollObj = readObject(obj, "roll");
        DeathCinematicState.Roll roll = new DeathCinematicState.Roll(
            readDouble(rollObj, "probability", 0.0),
            readDouble(rollObj, "threshold", 0.0),
            readDouble(rollObj, "luck_value", 0.0),
            DeathCinematicState.RollResult.fromWire(readString(rollObj, "result"))
        );

        return new DeathCinematicState(
            true,
            readString(obj, "character_id"),
            DeathCinematicState.Phase.fromWire(readString(obj, "phase")),
            readLong(obj, "phase_tick", 0L),
            readLong(obj, "phase_duration_ticks", 1L),
            readLong(obj, "total_elapsed_ticks", 0L),
            readLong(obj, "total_duration_ticks", 1L),
            roll,
            readStringArray(obj, "insight_text"),
            readBoolean(obj, "is_final", false),
            (int) readLong(obj, "death_number", 0L),
            readString(obj, "zone_kind"),
            readBoolean(obj, "tsy_death", false),
            readLong(obj, "rebirth_weakened_ticks", 0L),
            readBoolean(obj, "skip_predeath", false),
            receivedAtMillis
        );
    }

    private static String readString(JsonObject obj, String field) {
        if (obj == null) return "";
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return "";
        JsonPrimitive primitive = el.getAsJsonPrimitive();
        return primitive.isString() ? primitive.getAsString() : "";
    }

    private static boolean readBoolean(JsonObject obj, String field, boolean fallback) {
        if (obj == null) return fallback;
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive primitive = el.getAsJsonPrimitive();
        if (primitive.isBoolean()) return primitive.getAsBoolean();
        if (primitive.isNumber()) return primitive.getAsDouble() != 0.0;
        return fallback;
    }

    private static long readLong(JsonObject obj, String field, long fallback) {
        if (obj == null) return fallback;
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive primitive = el.getAsJsonPrimitive();
        if (!primitive.isNumber()) return fallback;
        double value = primitive.getAsDouble();
        if (!Double.isFinite(value)) return fallback;
        return Math.max(0L, Math.round(value));
    }

    private static double readDouble(JsonObject obj, String field, double fallback) {
        if (obj == null) return fallback;
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive primitive = el.getAsJsonPrimitive();
        if (!primitive.isNumber()) return fallback;
        double value = primitive.getAsDouble();
        return Double.isFinite(value) ? value : fallback;
    }

    private static JsonObject readObject(JsonObject obj, String field) {
        if (obj == null) return null;
        JsonElement el = obj.get(field);
        return el != null && el.isJsonObject() ? el.getAsJsonObject() : null;
    }

    private static List<String> readStringArray(JsonObject obj, String field) {
        JsonElement el = obj == null ? null : obj.get(field);
        if (el == null || !el.isJsonArray()) {
            return List.of();
        }
        JsonArray arr = el.getAsJsonArray();
        List<String> out = new ArrayList<>();
        for (JsonElement entry : arr) {
            if (entry != null && entry.isJsonPrimitive() && entry.getAsJsonPrimitive().isString()) {
                String value = entry.getAsString();
                if (value != null && !value.isBlank()) {
                    out.add(value);
                }
            }
        }
        return out;
    }
}
