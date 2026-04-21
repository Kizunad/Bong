package com.bong.client.combat.handler;

import com.bong.client.combat.store.DamageFloaterStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

/**
 * Handles {@code combat_event} payloads (plan §U1).
 * <p>Schema (minimal; extensible):
 * <pre>{@code
 * {
 *   "events": [
 *     { "kind": "hit" | "crit" | "block" | "heal" | "qi_damage",
 *       "amount": 12.5,
 *       "x": 0.0, "y": 0.0, "z": 0.0,
 *       "color": -65536,
 *       "text": "12"
 *     }
 *   ]
 * }
 * }</pre>
 * The handler mutates {@link DamageFloaterStore} in place and reports a no-op
 * dispatch (no ServerDataDispatch-level state to bubble up).
 */
public final class CombatEventHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        JsonElement eventsEl = payload.get("events");
        if (eventsEl == null || eventsEl.isJsonNull() || !eventsEl.isJsonArray()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring combat_event payload: 'events' array missing"
            );
        }

        JsonArray events = eventsEl.getAsJsonArray();
        int accepted = 0;
        long now = System.currentTimeMillis();
        for (JsonElement el : events) {
            if (el == null || !el.isJsonObject()) continue;
            JsonObject obj = el.getAsJsonObject();
            String kindWire = readString(obj, "kind");
            double x = readDouble(obj, "x", 0d);
            double y = readDouble(obj, "y", 0d);
            double z = readDouble(obj, "z", 0d);
            String text = readString(obj, "text");
            int color = (int) Math.round(readDouble(obj, "color", defaultColorFor(kindWire)));
            DamageFloaterStore.Kind kind = parseKind(kindWire);
            if (text == null || text.isEmpty()) {
                double amount = readDouble(obj, "amount", 0d);
                text = amount > 0 ? formatAmount(amount) : kindWire;
            }
            DamageFloaterStore.publish(
                new DamageFloaterStore.Floater(x, y, z, text, color, kind, now)
            );
            accepted++;
        }

        if (accepted == 0) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring combat_event payload: no valid events"
            );
        }
        return ServerDataDispatch.handled(
            envelope.type(),
            "Accepted " + accepted + " combat_event entries into DamageFloaterStore"
        );
    }

    private static DamageFloaterStore.Kind parseKind(String wire) {
        if (wire == null) return DamageFloaterStore.Kind.HIT;
        return switch (wire.trim().toLowerCase(java.util.Locale.ROOT)) {
            case "crit" -> DamageFloaterStore.Kind.CRIT;
            case "block" -> DamageFloaterStore.Kind.BLOCK;
            case "heal" -> DamageFloaterStore.Kind.HEAL;
            case "qi_damage" -> DamageFloaterStore.Kind.QI_DAMAGE;
            default -> DamageFloaterStore.Kind.HIT;
        };
    }

    private static int defaultColorFor(String wire) {
        return switch (wire == null ? "" : wire.toLowerCase(java.util.Locale.ROOT)) {
            case "crit" -> 0xFFFFC040;
            case "block" -> 0xFFA0A0A0;
            case "heal" -> 0xFF60E060;
            case "qi_damage" -> 0xFF80A0FF;
            default -> 0xFFE04040;
        };
    }

    private static String formatAmount(double amount) {
        if (Math.abs(amount - Math.rint(amount)) < 1e-6) {
            return Integer.toString((int) Math.rint(amount));
        }
        return String.format(java.util.Locale.ROOT, "%.1f", amount);
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
