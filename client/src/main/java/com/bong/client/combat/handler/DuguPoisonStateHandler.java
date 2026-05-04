package com.bong.client.combat.handler;

import com.bong.client.combat.store.DuguPoisonStateStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

/** Handles {@code dugu_poison_state} server-data payloads. */
public final class DuguPoisonStateHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        DuguPoisonStateStore.State next = new DuguPoisonStateStore.State(
            readBoolean(payload, "active", false),
            readString(payload, "meridian_id"),
            readString(payload, "attacker"),
            readLong(payload, "attached_at_tick", 0L),
            (int) readLong(payload, "poisoner_realm_tier", 0L),
            readDouble(payload, "loss_per_tick", 0d),
            readDouble(payload, "flow_capacity_after", 0d),
            readDouble(payload, "qi_max_after", 0d),
            readLong(payload, "server_tick", 0L)
        );
        DuguPoisonStateStore.replace(next);
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied dugu_poison_state (active=" + next.active() + ", meridian=" + next.meridianId() + ")"
        );
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
