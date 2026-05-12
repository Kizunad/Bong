package com.bong.client.death;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.ArrayList;
import java.util.List;

public final class DeathCinematicPayloadParser {
    private static final long SUPPORTED_VERSION = 1L;

    private DeathCinematicPayloadParser() {}

    public static DeathCinematicState parse(JsonObject obj) {
        return parse(obj, System.currentTimeMillis());
    }

    static DeathCinematicState parse(JsonObject obj, long receivedAtMillis) {
        if (obj == null || !hasSupportedVersion(obj)) {
            return DeathCinematicState.INACTIVE;
        }
        JsonObject rollObj = readObject(obj, "roll");
        DeathCinematicState.Roll roll = new DeathCinematicState.Roll(
            readDouble(rollObj, "probability", 0.0),
            readDouble(rollObj, "threshold", 0.0),
            readDouble(rollObj, "luck_value", 0.0),
            DeathCinematicState.RollResult.fromWire(readString(rollObj, "result"))
        );
        String characterId = readString(obj, "character_id");
        if (characterId.isBlank()) {
            return DeathCinematicState.INACTIVE;
        }

        return new DeathCinematicState(
            true,
            characterId,
            DeathCinematicState.Phase.fromWire(readString(obj, "phase")),
            readLong(obj, "phase_tick", 0L),
            readDurationTicks(obj, "phase_duration_ticks"),
            readLong(obj, "total_elapsed_ticks", 0L),
            readDurationTicks(obj, "total_duration_ticks"),
            roll,
            readStringArray(obj, "insight_text"),
            readBoolean(obj, "is_final", false),
            readInt(obj, "death_number", 0),
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

    private static boolean hasSupportedVersion(JsonObject obj) {
        JsonElement el = obj.get("v");
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return false;
        JsonPrimitive primitive = el.getAsJsonPrimitive();
        if (!primitive.isNumber()) return false;
        double value = primitive.getAsDouble();
        return Double.isFinite(value)
            && Math.rint(value) == value
            && (long) value == SUPPORTED_VERSION;
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

    private static long readDurationTicks(JsonObject obj, String field) {
        return Math.max(1L, readLong(obj, field, 1L));
    }

    private static int readInt(JsonObject obj, String field, int fallback) {
        long value = readLong(obj, field, fallback);
        if (value > Integer.MAX_VALUE) return Integer.MAX_VALUE;
        return (int) value;
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
