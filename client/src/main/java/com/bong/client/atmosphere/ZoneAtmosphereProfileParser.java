package com.bong.client.atmosphere;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.JsonPrimitive;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

public final class ZoneAtmosphereProfileParser {
    private ZoneAtmosphereProfileParser() {
    }

    public static ParseResult parse(String json, String fallbackZoneId) {
        try {
            JsonObject root = JsonParser.parseString(json).getAsJsonObject();
            String zoneId = readString(root, "zone_id", fallbackZoneId);
            int fogColor = readColor(root, "fog_color");
            double fogDensity = readDouble(root, "fog_density", 0.0);
            int skyTint = readColor(root, "sky_tint");
            ZoneAtmosphereProfile.TransitionFx transitionFx = readTransition(root);
            String ambientRecipeId = readString(root, "ambient_recipe_id", "");
            List<ZoneAtmosphereProfile.ParticleConfig> particles = readParticles(root);
            return ParseResult.success(new ZoneAtmosphereProfile(
                zoneId,
                fogColor,
                fogDensity,
                particles,
                skyTint,
                transitionFx,
                ambientRecipeId
            ));
        } catch (RuntimeException ex) {
            return ParseResult.error(ex.getMessage() == null ? ex.getClass().getSimpleName() : ex.getMessage());
        }
    }

    private static List<ZoneAtmosphereProfile.ParticleConfig> readParticles(JsonObject root) {
        JsonElement plural = root.get("ambient_particles");
        if (plural != null && plural.isJsonArray()) {
            JsonArray array = plural.getAsJsonArray();
            List<ZoneAtmosphereProfile.ParticleConfig> result = new ArrayList<>(array.size());
            for (JsonElement element : array) {
                result.add(readParticle(element.getAsJsonObject()));
            }
            return result;
        }
        JsonElement single = root.get("ambient_particle");
        if (single == null || single.isJsonNull()) {
            return List.of();
        }
        return List.of(readParticle(single.getAsJsonObject()));
    }

    private static ZoneAtmosphereProfile.ParticleConfig readParticle(JsonObject object) {
        return new ZoneAtmosphereProfile.ParticleConfig(
            readString(object, "type", "cloud256_dust"),
            readColor(object, "tint"),
            readDouble(object, "density", 0.0),
            readDrift(object, 0),
            readDrift(object, 1),
            readDrift(object, 2),
            readInt(object, "interval_ticks", 20)
        );
    }

    private static ZoneAtmosphereProfile.TransitionFx readTransition(JsonObject root) {
        String raw = readString(root, "entry_transition_fx", "NONE");
        try {
            return ZoneAtmosphereProfile.TransitionFx.valueOf(raw.trim().toUpperCase(Locale.ROOT));
        } catch (IllegalArgumentException ex) {
            throw new IllegalArgumentException("unknown entry_transition_fx: " + raw, ex);
        }
    }

    private static double readDrift(JsonObject object, int index) {
        JsonElement drift = object.get("drift");
        if (drift != null && drift.isJsonArray()) {
            JsonArray array = drift.getAsJsonArray();
            if (array.size() != 3) {
                throw new IllegalArgumentException("drift must be a vec3");
            }
            return array.get(index).getAsDouble();
        }
        if (index == 1 && object.has("vertical_drift")) {
            return readDouble(object, "vertical_drift", 0.0);
        }
        if (object.has("drift_speed")) {
            double speed = readDouble(object, "drift_speed", 0.0);
            return index == 0 ? speed : 0.0;
        }
        return 0.0;
    }

    static int readColor(JsonObject object, String field) {
        JsonElement element = object.get(field);
        if (element == null || element.isJsonNull()) {
            throw new IllegalArgumentException(field + " is required");
        }
        if (element.isJsonPrimitive()) {
            JsonPrimitive primitive = element.getAsJsonPrimitive();
            if (primitive.isString()) {
                return parseHexColor(primitive.getAsString(), field);
            }
            if (primitive.isNumber()) {
                return primitive.getAsInt() & 0x00FFFFFF;
            }
        }
        if (element.isJsonArray()) {
            JsonArray array = element.getAsJsonArray();
            if (array.size() != 3) {
                throw new IllegalArgumentException(field + " must be rgb tuple");
            }
            int r = readChannel(array, 0, field);
            int g = readChannel(array, 1, field);
            int b = readChannel(array, 2, field);
            return (r << 16) | (g << 8) | b;
        }
        throw new IllegalArgumentException(field + " must be #RRGGBB or rgb tuple");
    }

    private static int parseHexColor(String raw, String field) {
        String text = raw == null ? "" : raw.trim();
        if (text.startsWith("#")) {
            text = text.substring(1);
        }
        if (text.length() == 8) {
            text = text.substring(2);
        }
        if (text.length() != 6) {
            throw new IllegalArgumentException(field + " must be #RRGGBB");
        }
        return Integer.parseInt(text, 16) & 0x00FFFFFF;
    }

    private static int readChannel(JsonArray array, int index, String field) {
        int channel = array.get(index).getAsInt();
        if (channel < 0 || channel > 255) {
            throw new IllegalArgumentException(field + " channel out of range: " + channel);
        }
        return channel;
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

    private static int readInt(JsonObject object, String field, int fallback) {
        JsonElement element = object.get(field);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return fallback;
        }
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        return primitive.isNumber() ? primitive.getAsInt() : fallback;
    }

    public record ParseResult(ZoneAtmosphereProfile profile, String error) {
        static ParseResult success(ZoneAtmosphereProfile profile) {
            return new ParseResult(profile, "");
        }

        static ParseResult error(String error) {
            return new ParseResult(null, error == null ? "parse error" : error);
        }

        public boolean ok() {
            return profile != null;
        }
    }
}
