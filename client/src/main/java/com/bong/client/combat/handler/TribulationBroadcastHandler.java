package com.bong.client.combat.handler;

import com.bong.client.combat.store.TribulationBroadcastStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

/**
 * Handles {@code tribulation_broadcast} payloads (plan §U6). Last-write-wins.
 * <pre>{@code
 * { "active": true,
 *   "actor_name": "某某", "stage": "warn",
 *   "world_x": 120.0, "world_z": -45.0,
 *   "expires_at_ms": 1712999999999,
 *   "spectate_invite": true, "spectate_distance": 32.0 }
 * }</pre>
 */
public final class TribulationBroadcastHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        boolean active = readBoolean(payload, "active", true);
        if (!active) {
            TribulationBroadcastStore.clear();
            return ServerDataDispatch.handled(envelope.type(), "tribulation cleared");
        }
        TribulationBroadcastStore.replace(new TribulationBroadcastStore.State(
            true,
            readString(payload, "actor_name"),
            readString(payload, "stage"),
            readDouble(payload, "world_x", 0d),
            readDouble(payload, "world_z", 0d),
            (long) readDouble(payload, "expires_at_ms", 0d),
            readBoolean(payload, "spectate_invite", false),
            readDouble(payload, "spectate_distance", 0d)
        ));
        return ServerDataDispatch.handled(envelope.type(), "tribulation active");
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
        double v = p.getAsDouble();
        return Double.isFinite(v) ? v : fallback;
    }
}
