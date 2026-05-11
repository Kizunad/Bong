package com.bong.client.cultivation;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.Locale;

public record BreakthroughCinematicPayload(
    String actorId,
    Phase phase,
    int phaseTick,
    int phaseDurationTicks,
    String realmFrom,
    String realmTo,
    Result result,
    boolean interrupted,
    double worldX,
    double worldY,
    double worldZ,
    double visibleRadiusBlocks,
    boolean global,
    boolean distantBillboard,
    double particleDensity,
    double intensity,
    String seasonOverlay,
    String style,
    long atTick
) {
    public BreakthroughCinematicPayload {
        actorId = sanitize(actorId, "unknown");
        phase = phase == null ? Phase.PRELUDE : phase;
        phaseTick = Math.max(0, phaseTick);
        phaseDurationTicks = Math.max(1, phaseDurationTicks);
        realmFrom = sanitize(realmFrom, "Awaken");
        realmTo = sanitize(realmTo, "Induce");
        result = result == null ? Result.PENDING : result;
        worldX = finite(worldX, 0.0);
        worldY = finite(worldY, 0.0);
        worldZ = finite(worldZ, 0.0);
        visibleRadiusBlocks = Math.max(1.0, finite(visibleRadiusBlocks, 1.0));
        particleDensity = clamp(finite(particleDensity, 1.0), 0.0, 8.0);
        intensity = clamp(finite(intensity, 0.4), 0.0, 1.0);
        seasonOverlay = sanitize(seasonOverlay, "adaptive");
        style = sanitize(style, "fresh_spiral");
        atTick = Math.max(0L, atTick);
    }

    public static BreakthroughCinematicPayload parse(JsonObject payload) {
        if (payload == null) return null;
        String actorId = readString(payload, "actor_id");
        Phase phase = Phase.fromWire(readString(payload, "phase"));
        Integer phaseTick = readInt(payload, "phase_tick");
        Integer phaseDurationTicks = readInt(payload, "phase_duration_ticks");
        String realmFrom = readString(payload, "realm_from");
        String realmTo = readString(payload, "realm_to");
        Result result = Result.fromWire(readString(payload, "result"));
        Boolean interrupted = readBoolean(payload, "interrupted");
        double[] worldPos = readVec3(payload, "world_pos");
        Double visibleRadiusBlocks = readDouble(payload, "visible_radius_blocks");
        Boolean global = readBoolean(payload, "global");
        Boolean distantBillboard = readBoolean(payload, "distant_billboard");
        Double particleDensity = readDouble(payload, "particle_density");
        Double intensity = readDouble(payload, "intensity");
        String seasonOverlay = readString(payload, "season_overlay");
        String style = readString(payload, "style");
        Long atTick = readLong(payload, "at_tick");

        if (actorId == null || phase == null || phaseTick == null || phaseDurationTicks == null
            || realmFrom == null || realmTo == null || result == null || interrupted == null
            || worldPos == null || visibleRadiusBlocks == null || global == null || distantBillboard == null
            || particleDensity == null || intensity == null || atTick == null) {
            return null;
        }

        return new BreakthroughCinematicPayload(
            actorId,
            phase,
            phaseTick,
            phaseDurationTicks,
            realmFrom,
            realmTo,
            result,
            interrupted,
            worldPos[0],
            worldPos[1],
            worldPos[2],
            visibleRadiusBlocks,
            global,
            distantBillboard,
            particleDensity,
            intensity,
            seasonOverlay,
            style,
            atTick
        );
    }

    public enum Phase {
        PRELUDE("prelude"),
        CHARGE("charge"),
        CATALYZE("catalyze"),
        APEX("apex"),
        AFTERMATH("aftermath");

        private final String wireName;

        Phase(String wireName) {
            this.wireName = wireName;
        }

        public String wireName() {
            return wireName;
        }

        public static Phase fromWire(String raw) {
            String normalized = normalize(raw);
            for (Phase phase : values()) {
                if (phase.wireName.equals(normalized)) return phase;
            }
            return null;
        }
    }

    public enum Result {
        PENDING("pending"),
        SUCCESS("success"),
        FAILURE("failure"),
        INTERRUPTED("interrupted");

        private final String wireName;

        Result(String wireName) {
            this.wireName = wireName;
        }

        public boolean failed() {
            return this == FAILURE || this == INTERRUPTED;
        }

        public String wireName() {
            return wireName;
        }

        public static Result fromWire(String raw) {
            String normalized = normalize(raw);
            for (Result result : values()) {
                if (result.wireName.equals(normalized)) return result;
            }
            return null;
        }
    }

    private static String sanitize(String value, String fallback) {
        if (value == null || value.isBlank()) return fallback;
        return value.trim();
    }

    private static String normalize(String raw) {
        return raw == null ? "" : raw.trim().toLowerCase(Locale.ROOT);
    }

    private static double finite(double value, double fallback) {
        return Double.isFinite(value) ? value : fallback;
    }

    private static double clamp(double value, double min, double max) {
        return Math.max(min, Math.min(max, value));
    }

    private static String readString(JsonObject obj, String field) {
        JsonPrimitive primitive = readPrimitive(obj, field);
        return primitive != null && primitive.isString() ? primitive.getAsString() : null;
    }

    private static Boolean readBoolean(JsonObject obj, String field) {
        JsonPrimitive primitive = readPrimitive(obj, field);
        return primitive != null && primitive.isBoolean() ? primitive.getAsBoolean() : null;
    }

    private static Integer readInt(JsonObject obj, String field) {
        JsonPrimitive primitive = readPrimitive(obj, field);
        return primitive != null && primitive.isNumber() ? primitive.getAsInt() : null;
    }

    private static Long readLong(JsonObject obj, String field) {
        JsonPrimitive primitive = readPrimitive(obj, field);
        return primitive != null && primitive.isNumber() ? primitive.getAsLong() : null;
    }

    private static Double readDouble(JsonObject obj, String field) {
        JsonPrimitive primitive = readPrimitive(obj, field);
        return primitive != null && primitive.isNumber() ? primitive.getAsDouble() : null;
    }

    private static double[] readVec3(JsonObject obj, String field) {
        JsonElement element = obj.get(field);
        if (element == null || !element.isJsonArray() || element.getAsJsonArray().size() != 3) {
            return null;
        }
        double[] values = new double[3];
        for (int i = 0; i < 3; i++) {
            JsonElement item = element.getAsJsonArray().get(i);
            if (!item.isJsonPrimitive() || !item.getAsJsonPrimitive().isNumber()) {
                return null;
            }
            values[i] = item.getAsDouble();
        }
        return values;
    }

    private static JsonPrimitive readPrimitive(JsonObject obj, String field) {
        JsonElement element = obj.get(field);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }
        return element.getAsJsonPrimitive();
    }
}
