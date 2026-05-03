package com.bong.client.combat.handler;

import com.bong.client.combat.store.CarrierStateStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

/** Handles anqi {@code carrier_state} server-data payloads. */
public final class CarrierStateHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        CarrierStateStore.State next = new CarrierStateStore.State(
            readPhase(payload, "phase"),
            (float) readDouble(payload, "progress", 0d),
            (float) readDouble(payload, "sealed_qi", 0d),
            (float) readDouble(payload, "sealed_qi_initial", 0d),
            readLong(payload, "half_life_remaining_ticks", 0L),
            readLong(payload, "item_instance_id", -1L)
        );
        CarrierStateStore.replace(next);
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied carrier_state (phase=" + next.phase() + ", sealed_qi=" + next.sealedQi() + ")"
        );
    }

    private static CarrierStateStore.Phase readPhase(JsonObject obj, String field) {
        String raw = readString(obj, field, "idle");
        return switch (raw) {
            case "charging" -> CarrierStateStore.Phase.CHARGING;
            case "charged" -> CarrierStateStore.Phase.CHARGED;
            default -> CarrierStateStore.Phase.IDLE;
        };
    }

    private static String readString(JsonObject obj, String field, String fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive p = el.getAsJsonPrimitive();
        return p.isString() ? p.getAsString() : fallback;
    }

    private static double readDouble(JsonObject obj, String field, double fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isNumber()) return fallback;
        double value = p.getAsDouble();
        return Double.isFinite(value) ? value : fallback;
    }

    private static long readLong(JsonObject obj, String field, long fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isNumber()) return fallback;
        return p.getAsLong();
    }
}
