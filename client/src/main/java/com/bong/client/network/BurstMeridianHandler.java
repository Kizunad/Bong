package com.bong.client.network;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

public final class BurstMeridianHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        String skill = readString(payload, "skill");
        String caster = readString(payload, "caster");
        Long tick = readLong(payload, "tick");
        Double overloadRatio = readDouble(payload, "overload_ratio");
        Double integritySnapshot = readDouble(payload, "integrity_snapshot");

        if (skill == null || caster == null || tick == null || overloadRatio == null || integritySnapshot == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring burst_meridian_event payload: required fields missing or invalid"
            );
        }

        return ServerDataDispatch.handled(
            envelope.type(),
            "burst_meridian_event accepted (skill=" + skill + " caster=" + caster + " tick=" + tick + ")"
        );
    }

    private static String readString(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        JsonPrimitive p = el.getAsJsonPrimitive();
        return p.isString() ? p.getAsString() : null;
    }

    private static Long readLong(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        JsonPrimitive p = el.getAsJsonPrimitive();
        return p.isNumber() ? p.getAsLong() : null;
    }

    private static Double readDouble(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        JsonPrimitive p = el.getAsJsonPrimitive();
        return p.isNumber() ? p.getAsDouble() : null;
    }
}
