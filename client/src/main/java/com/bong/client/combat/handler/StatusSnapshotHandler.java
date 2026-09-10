package com.bong.client.combat.handler;

import com.bong.client.combat.store.StatusEffectStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.ArrayList;
import java.util.List;

/**
 * Handles {@code status_snapshot} payloads (plan §U2 / §2.5).
 * <p>Full replacement of {@link StatusEffectStore} contents.
 * <pre>{@code
 * {
 *   "effects": [
 *     { "id": "burn", "name": "烈焰灼烧", "kind": "dot", "stacks": 2,
 *       "remaining_ms": 5400, "source_color": -65536,
 *       "source_label": "僵尸狂战士", "dispel": 3 }
 *   ]
 * }
 * }</pre>
 */
public final class StatusSnapshotHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        JsonElement effectsEl = payload.get("effects");
        if (effectsEl == null || effectsEl.isJsonNull() || !effectsEl.isJsonArray()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring status_snapshot payload: 'effects' array missing"
            );
        }

        JsonArray effects = effectsEl.getAsJsonArray();
        List<StatusEffectStore.Effect> parsed = new ArrayList<>(effects.size());
        int invalid = 0;
        for (JsonElement el : effects) {
            if (el == null || !el.isJsonObject()) { invalid++; continue; }
            JsonObject obj = el.getAsJsonObject();
            String id = readString(obj, "id");
            String name = readString(obj, "name");
            if (name == null) name = id;
            String kindWire = readString(obj, "kind");
            int stacks = (int) Math.max(0L, Math.round(readDouble(obj, "stacks", 1d)));
            long remaining = (long) Math.max(0d, readDouble(obj, "remaining_ms", 0d));
            int color = (int) Math.round(readDouble(obj, "source_color", 0xFFA0A0A0));
            String sourceLabel = readString(obj, "source_label");
            int dispel = (int) Math.round(readDouble(obj, "dispel", 0d));
            if (id == null || id.isBlank()) { invalid++; continue; }
            parsed.add(new StatusEffectStore.Effect(
                id,
                name == null ? id : name,
                StatusEffectStore.Kind.fromWire(kindWire),
                stacks,
                remaining,
                color,
                sourceLabel == null ? "" : sourceLabel,
                dispel
            ));
        }

        StatusEffectStore.replace(parsed);
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied status_snapshot with " + parsed.size() + " effects ("
                + invalid + " invalid skipped)"
        );
    }

    private static String readString(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        JsonPrimitive p = el.getAsJsonPrimitive();
        return p.isString() ? p.getAsString() : null;
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
