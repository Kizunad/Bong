package com.bong.client.combat.handler;

import com.bong.client.combat.juice.CombatJuiceEvent;
import com.bong.client.combat.juice.CombatJuiceSystem;
import com.bong.client.combat.juice.CombatJuiceTier;
import com.bong.client.combat.juice.CombatSchool;
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
            double amount = readDouble(obj, "amount", 0d);
            double x = readDouble(obj, "x", 0d);
            double y = readDouble(obj, "y", 0d);
            double z = readDouble(obj, "z", 0d);
            String text = readString(obj, "text");
            if (text != null) {
                text = text.trim();
            }
            int color = (int) Math.round(readDouble(obj, "color", defaultColorFor(kindWire)));
            DamageFloaterStore.Kind kind = parseKind(kindWire);
            if (text == null || text.isBlank()) {
                text = amount > 0 ? formatAmount(amount) : kindFallback(kindWire);
            }
            if (text.isBlank()) {
                continue;
            }
            DamageFloaterStore.publish(
                new DamageFloaterStore.Floater(x, y, z, text, color, kind, now)
            );
            CombatJuiceSystem.accept(toJuiceEvent(obj, kindWire, amount, now));
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

    private static String kindFallback(String wire) {
        return wire == null ? "" : wire.trim();
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

    private static CombatJuiceEvent toJuiceEvent(JsonObject obj, String kindWire, double amount, long now) {
        CombatJuiceEvent.Kind juiceKind = juiceKind(obj, kindWire);
        CombatSchool school = CombatSchool.fromWire(firstString(obj, "school", "style", "skill_school"));
        CombatJuiceTier tier = CombatJuiceTier.fromCombatEvent(kindWire, amount, readString(obj, "tier"));
        return new CombatJuiceEvent(
            juiceKind,
            school,
            tier,
            firstString(obj, "attacker_uuid", "source_uuid", "caster_uuid"),
            firstString(obj, "target_uuid", "defender_uuid", "victim_uuid", "entity_uuid"),
            readString(obj, "local_player_uuid"),
            firstString(obj, "victim_name", "target_name", "entity_name"),
            firstDouble(obj, 0.0, "direction_x", "dir_x", "dx"),
            firstDouble(obj, 1.0, "direction_z", "dir_z", "dz"),
            readBoolean(obj, "rare_drop") || readBoolean(obj, "is_rare_drop"),
            now
        );
    }

    private static CombatJuiceEvent.Kind juiceKind(JsonObject obj, String kindWire) {
        String normalized = kindWire == null ? "" : kindWire.trim().toLowerCase(java.util.Locale.ROOT);
        if (readBoolean(obj, "kill") || readBoolean(obj, "is_kill")) {
            return CombatJuiceEvent.Kind.KILL;
        }
        if (readBoolean(obj, "perfect") || readBoolean(obj, "perfect_parry")) {
            return CombatJuiceEvent.Kind.PERFECT_PARRY;
        }
        return switch (normalized) {
            case "qi_collision", "qi_collision_event" -> CombatJuiceEvent.Kind.QI_COLLISION;
            case "full_power_release", "full_charge", "charge_release", "release" -> CombatJuiceEvent.Kind.FULL_CHARGE;
            case "overload", "overload_tear", "meridian_overload" -> CombatJuiceEvent.Kind.OVERLOAD;
            case "parry", "block" -> CombatJuiceEvent.Kind.PARRY;
            case "perfect_parry" -> CombatJuiceEvent.Kind.PERFECT_PARRY;
            case "dodge" -> CombatJuiceEvent.Kind.DODGE;
            case "kill", "death" -> CombatJuiceEvent.Kind.KILL;
            case "wound", "wounds" -> CombatJuiceEvent.Kind.WOUND;
            default -> CombatJuiceEvent.Kind.HIT;
        };
    }

    private static String firstString(JsonObject obj, String... fields) {
        for (String field : fields) {
            String value = readString(obj, field);
            if (value != null && !value.isBlank()) {
                return value;
            }
        }
        return "";
    }

    private static double firstDouble(JsonObject obj, double fallback, String... fields) {
        for (String field : fields) {
            JsonElement el = obj.get(field);
            if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) {
                continue;
            }
            JsonPrimitive p = el.getAsJsonPrimitive();
            if (!p.isNumber()) {
                continue;
            }
            double v = p.getAsDouble();
            if (Double.isFinite(v)) {
                return v;
            }
        }
        return fallback;
    }

    private static boolean readBoolean(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) {
            return false;
        }
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (p.isBoolean()) {
            return p.getAsBoolean();
        }
        if (p.isNumber()) {
            return p.getAsInt() != 0;
        }
        if (p.isString()) {
            return Boolean.parseBoolean(p.getAsString());
        }
        return false;
    }
}
