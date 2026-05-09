package com.bong.client.environment;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import java.util.ArrayList;
import java.util.List;

public final class EnvironmentEffectParser {
    private EnvironmentEffectParser() {
    }

    public static ParseResult parse(String json) {
        try {
            JsonObject root = JsonParser.parseString(json).getAsJsonObject();
            int version = readInt(root, "v");
            String zoneId = readString(root, "zone_id");
            long generation = readLong(root, "generation");
            JsonArray rawEffects = root.getAsJsonArray("effects");
            if (rawEffects == null) {
                return ParseResult.error("effects must be an array");
            }
            List<EnvironmentEffect> effects = new ArrayList<>(rawEffects.size());
            for (JsonElement element : rawEffects) {
                effects.add(parseEffect(element.getAsJsonObject()));
            }
            ZoneEnvironmentState state = new ZoneEnvironmentState(version, zoneId, effects, generation);
            if (!state.valid()) {
                return ParseResult.error("invalid zone environment state");
            }
            return ParseResult.success(state);
        } catch (RuntimeException ex) {
            return ParseResult.error(ex.getMessage() == null ? ex.getClass().getSimpleName() : ex.getMessage());
        }
    }

    private static EnvironmentEffect parseEffect(JsonObject object) {
        String kind = readString(object, "kind");
        return switch (kind) {
            case "tornado_column" -> new EnvironmentEffect.TornadoColumn(
                vec(object, "center", 0),
                vec(object, "center", 1),
                vec(object, "center", 2),
                readDouble(object, "radius"),
                readDouble(object, "height"),
                readDouble(object, "particle_density")
            );
            case "lightning_pillar" -> new EnvironmentEffect.LightningPillar(
                vec(object, "center", 0),
                vec(object, "center", 1),
                vec(object, "center", 2),
                readDouble(object, "radius"),
                readDouble(object, "strike_rate_per_min")
            );
            case "ash_fall" -> new EnvironmentEffect.AshFall(
                vec(object, "aabb_min", 0),
                vec(object, "aabb_min", 1),
                vec(object, "aabb_min", 2),
                vec(object, "aabb_max", 0),
                vec(object, "aabb_max", 1),
                vec(object, "aabb_max", 2),
                readDouble(object, "density")
            );
            case "fog_veil" -> new EnvironmentEffect.FogVeil(
                vec(object, "aabb_min", 0),
                vec(object, "aabb_min", 1),
                vec(object, "aabb_min", 2),
                vec(object, "aabb_max", 0),
                vec(object, "aabb_max", 1),
                vec(object, "aabb_max", 2),
                rgb(object, "tint_rgb"),
                readDouble(object, "density")
            );
            case "dust_devil" -> new EnvironmentEffect.DustDevil(
                vec(object, "center", 0),
                vec(object, "center", 1),
                vec(object, "center", 2),
                readDouble(object, "radius"),
                readDouble(object, "height")
            );
            case "ember_drift" -> new EnvironmentEffect.EmberDrift(
                vec(object, "aabb_min", 0),
                vec(object, "aabb_min", 1),
                vec(object, "aabb_min", 2),
                vec(object, "aabb_max", 0),
                vec(object, "aabb_max", 1),
                vec(object, "aabb_max", 2),
                readDouble(object, "density"),
                readDouble(object, "glow")
            );
            case "heat_haze" -> new EnvironmentEffect.HeatHaze(
                vec(object, "aabb_min", 0),
                vec(object, "aabb_min", 1),
                vec(object, "aabb_min", 2),
                vec(object, "aabb_max", 0),
                vec(object, "aabb_max", 1),
                vec(object, "aabb_max", 2),
                readDouble(object, "distortion_strength")
            );
            case "snow_drift" -> new EnvironmentEffect.SnowDrift(
                vec(object, "aabb_min", 0),
                vec(object, "aabb_min", 1),
                vec(object, "aabb_min", 2),
                vec(object, "aabb_max", 0),
                vec(object, "aabb_max", 1),
                vec(object, "aabb_max", 2),
                readDouble(object, "density"),
                vec(object, "wind_dir", 0),
                vec(object, "wind_dir", 1),
                vec(object, "wind_dir", 2)
            );
            default -> throw new IllegalArgumentException("unknown environment effect kind: " + kind);
        };
    }

    private static String readString(JsonObject object, String field) {
        JsonElement value = object.get(field);
        if (value == null || value.isJsonNull()) {
            throw new IllegalArgumentException(field + " is required");
        }
        return value.getAsString();
    }

    private static int readInt(JsonObject object, String field) {
        JsonElement value = object.get(field);
        if (value == null || value.isJsonNull()) {
            throw new IllegalArgumentException(field + " is required");
        }
        return value.getAsInt();
    }

    private static long readLong(JsonObject object, String field) {
        JsonElement value = object.get(field);
        if (value == null || value.isJsonNull()) {
            throw new IllegalArgumentException(field + " is required");
        }
        return value.getAsLong();
    }

    private static double readDouble(JsonObject object, String field) {
        JsonElement value = object.get(field);
        if (value == null || value.isJsonNull()) {
            throw new IllegalArgumentException(field + " is required");
        }
        return value.getAsDouble();
    }

    private static double vec(JsonObject object, String field, int index) {
        JsonArray array = object.getAsJsonArray(field);
        if (array == null || array.size() != 3) {
            throw new IllegalArgumentException(field + " must be a vec3");
        }
        return array.get(index).getAsDouble();
    }

    private static int rgb(JsonObject object, String field) {
        JsonArray array = object.getAsJsonArray(field);
        if (array == null || array.size() != 3) {
            throw new IllegalArgumentException(field + " must be rgb tuple");
        }
        int r = clampColor(array.get(0).getAsInt());
        int g = clampColor(array.get(1).getAsInt());
        int b = clampColor(array.get(2).getAsInt());
        return (r << 16) | (g << 8) | b;
    }

    private static int clampColor(int channel) {
        return Math.max(0, Math.min(255, channel));
    }

    public record ParseResult(ZoneEnvironmentState state, String error) {
        public static ParseResult success(ZoneEnvironmentState state) {
            return new ParseResult(state, "");
        }

        public static ParseResult error(String error) {
            return new ParseResult(null, error == null ? "parse error" : error);
        }

        public boolean ok() {
            return state != null;
        }
    }
}
