package com.bong.client.combat.handler;

import com.bong.client.combat.store.WoundsStore;
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
 * Handles {@code wounds_snapshot} payloads (plan §U1 / §2.3). Full replacement
 * of {@link WoundsStore}.
 * <pre>{@code
 * {
 *   "wounds": [
 *     { "part": "chest", "kind": "cut", "severity": 0.6,
 *       "state": "bleeding", "infection": 0.1, "scar": false,
 *       "updated_at_ms": 1712999999999 }
 *   ]
 * }
 * }</pre>
 */
public final class WoundsSnapshotHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        JsonElement woundsEl = payload.get("wounds");
        if (woundsEl == null || woundsEl.isJsonNull() || !woundsEl.isJsonArray()) {
            WoundsStore.replace(List.of());
            return ServerDataDispatch.handled(envelope.type(), "wounds_snapshot cleared (no wounds array)");
        }
        JsonArray arr = woundsEl.getAsJsonArray();
        List<WoundsStore.Wound> parsed = new ArrayList<>(arr.size());
        int invalid = 0;
        for (JsonElement el : arr) {
            if (el == null || !el.isJsonObject()) { invalid++; continue; }
            JsonObject obj = el.getAsJsonObject();
            String part = readString(obj, "part");
            if (part == null || part.isBlank()) { invalid++; continue; }
            parsed.add(new WoundsStore.Wound(
                part,
                readString(obj, "kind"),
                (float) readDouble(obj, "severity", 0d),
                WoundsStore.HealingState.fromWire(readString(obj, "state")),
                (float) readDouble(obj, "infection", 0d),
                readBoolean(obj, "scar", false),
                (long) readDouble(obj, "updated_at_ms", 0d)
            ));
        }
        WoundsStore.replace(parsed);
        // Mirror into the existing inspect-screen body store so伤口层 picks it up.
        com.bong.client.combat.inspect.WoundLayerBinding.apply();
        return ServerDataDispatch.handled(
            envelope.type(),
            "wounds_snapshot applied (" + parsed.size() + " wounds, " + invalid + " invalid)"
        );
    }

    private static String readString(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        JsonPrimitive p = el.getAsJsonPrimitive();
        return p.isString() ? p.getAsString() : null;
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
