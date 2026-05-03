package com.bong.client.combat.handler;

import com.bong.client.combat.store.VortexStateStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

/** Handles woliu {@code vortex_state} server-data payloads. */
public final class VortexStateHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        VortexStateStore.State next = new VortexStateStore.State(
            readBoolean(payload, "active", false),
            (float) readDouble(payload, "radius", 0d),
            (float) readDouble(payload, "delta", 0d),
            (float) readDouble(payload, "env_qi_at_cast", 0d),
            readLong(payload, "maintain_remaining_ticks", 0L),
            (int) Math.min(Integer.MAX_VALUE, readLong(payload, "intercepted_count", 0L))
        );
        VortexStateStore.replace(next);
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied vortex_state (active=" + next.active() + ", delta=" + next.delta() + ")"
        );
    }

    private static boolean readBoolean(JsonObject obj, String field, boolean fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (p.isBoolean()) return p.getAsBoolean();
        if (p.isNumber()) return p.getAsDouble() != 0d;
        return fallback;
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
        long value = p.getAsLong();
        return value < 0 ? fallback : value;
    }
}
