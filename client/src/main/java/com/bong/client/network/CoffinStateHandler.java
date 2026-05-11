package com.bong.client.network;

import com.bong.client.hud.CoffinStateStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

public final class CoffinStateHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        if (!"coffin_state".equals(envelope.type())) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring coffin payload: unsupported type '" + envelope.type() + "'"
            );
        }

        Boolean inCoffin = readBoolean(envelope.payload(), "in_coffin");
        Double multiplier = readDouble(envelope.payload(), "lifespan_rate_multiplier");
        if (inCoffin == null || multiplier == null || multiplier <= 0.0) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring coffin_state payload: required fields missing or invalid"
            );
        }

        CoffinStateStore.replace(new CoffinStateStore.State(inCoffin, multiplier));
        return ServerDataDispatch.handled(
            envelope.type(),
            "coffin_state accepted (in_coffin=" + inCoffin + " multiplier=" + multiplier + ")"
        );
    }

    private static Boolean readBoolean(JsonObject obj, String field) {
        JsonElement element = obj.get(field);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        return primitive.isBoolean() ? primitive.getAsBoolean() : null;
    }

    private static Double readDouble(JsonObject obj, String field) {
        JsonElement element = obj.get(field);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return null;
        }
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isNumber()) {
            return null;
        }
        double value = primitive.getAsDouble();
        return Double.isFinite(value) ? value : null;
    }
}
