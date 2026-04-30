package com.bong.client.visual.realm_vision;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

public final class RealmVisionStateReducer {
    private RealmVisionStateReducer() {
    }

    public static RealmVisionState apply(RealmVisionState previous, JsonObject payload) {
        return apply(previous, payload, 0L);
    }

    public static RealmVisionState apply(RealmVisionState previous, JsonObject payload, long startedAtTick) {
        if (payload == null) {
            return previous == null ? RealmVisionState.empty() : previous;
        }
        RealmVisionState safePrevious = previous == null ? RealmVisionState.empty() : previous;
        RealmVisionCommand command = new RealmVisionCommand(
            readDouble(payload, "fog_start"),
            readDouble(payload, "fog_end"),
            (int) readDouble(payload, "fog_color_rgb"),
            FogShape.fromWire(readString(payload, "fog_shape")),
            readDouble(payload, "vignette_alpha"),
            (int) readDouble(payload, "tint_color_argb"),
            readDouble(payload, "particle_density"),
            readDouble(payload, "post_fx_sharpen")
        );
        int transitionTicks = Math.max(0, (int) readDouble(payload, "transition_ticks"));
        int chunks = Math.max(0, (int) readDouble(payload, "server_view_distance_chunks"));
        return new RealmVisionState(command, safePrevious.current(), transitionTicks, 0, startedAtTick, chunks);
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
