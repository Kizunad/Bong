package com.bong.client.combat.handler;

import com.bong.client.combat.store.HalfStepRechallengeStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

/**
 * Handles {@code halfstep_rechallenge} payloads (plan-halfstep-rechallenge-integration-v1 P0).
 * Last-write-wins. {@code active=false} → {@link HalfStepRechallengeStore#clear()}.
 *
 * <pre>{@code
 * { "active": true,
 *   "char_id": "offline:Azure",
 *   "rechallenge_window_until": 4032000,
 *   "at_tick": 10000 }
 * }</pre>
 */
public final class HalfStepRechallengeHandler implements ServerDataHandler {

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        boolean active = readBoolean(payload, "active", true);
        if (!active) {
            HalfStepRechallengeStore.clear();
            return ServerDataDispatch.handled(envelope.type(), "halfstep rechallenge cleared");
        }
        String charId = readString(payload, "char_id");
        long windowUntilTick = readLong(payload, "rechallenge_window_until", 0L);
        long atTick = readLong(payload, "at_tick", 0L);
        long nowMs = System.currentTimeMillis();
        HalfStepRechallengeStore.replace(new HalfStepRechallengeStore.State(
            true, charId, windowUntilTick, atTick, nowMs
        ));
        return ServerDataDispatch.handled(envelope.type(), "halfstep rechallenge active");
    }

    private static String readString(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return "";
        JsonPrimitive p = el.getAsJsonPrimitive();
        return p.isString() ? p.getAsString() : "";
    }

    private static boolean readBoolean(JsonObject obj, String field, boolean fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (p.isBoolean()) return p.getAsBoolean();
        if (p.isNumber()) return p.getAsDouble() != 0d;
        return fallback;
    }

    private static long readLong(JsonObject obj, String field, long fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isNumber()) return fallback;
        double v = p.getAsDouble();
        return Double.isFinite(v) ? (long) v : fallback;
    }
}
