package com.bong.client.combat.handler;

import com.bong.client.combat.store.TribulationStateStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.ArrayList;
import java.util.List;

/** Handles {@code tribulation_state} snapshots. */
public final class TribulationStateHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        boolean active = readBoolean(payload, "active", false);
        TribulationStateStore.State state = new TribulationStateStore.State(
            active,
            readString(payload, "char_id"),
            readString(payload, "actor_name"),
            readString(payload, "kind"),
            readString(payload, "phase"),
            readDouble(payload, "world_x", 0d),
            readDouble(payload, "world_z", 0d),
            readInt(payload, "wave_current", 0),
            readInt(payload, "wave_total", 0),
            readLong(payload, "started_tick", 0L),
            readLong(payload, "phase_started_tick", 0L),
            readLong(payload, "next_wave_tick", 0L),
            readBoolean(payload, "failed", false),
            readBoolean(payload, "half_step_on_success", false),
            readStringList(payload, "participants"),
            readString(payload, "result")
        );
        if (active) {
            TribulationStateStore.replace(state);
            return ServerDataDispatch.handled(envelope.type(), "tribulation state updated");
        }
        TribulationStateStore.clear(state);
        return ServerDataDispatch.handled(envelope.type(), "tribulation state cleared");
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

    private static int readInt(JsonObject obj, String field, int fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isNumber()) return fallback;
        double value = p.getAsDouble();
        if (!Double.isFinite(value)) return fallback;
        return Math.max(0, (int) value);
    }

    private static long readLong(JsonObject obj, String field, long fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isNumber()) return fallback;
        double value = p.getAsDouble();
        if (!Double.isFinite(value)) return fallback;
        return Math.max(0L, (long) value);
    }

    private static double readDouble(JsonObject obj, String field, double fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isNumber()) return fallback;
        double value = p.getAsDouble();
        return Double.isFinite(value) ? value : fallback;
    }

    private static List<String> readStringList(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonArray()) return List.of();
        JsonArray array = el.getAsJsonArray();
        List<String> values = new ArrayList<>();
        for (JsonElement item : array) {
            if (item == null || item.isJsonNull() || !item.isJsonPrimitive()) continue;
            JsonPrimitive primitive = item.getAsJsonPrimitive();
            if (primitive.isString()) values.add(primitive.getAsString());
        }
        return values;
    }
}
