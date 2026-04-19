package com.bong.client.combat.handler;

import com.bong.client.combat.CombatHudStateStore;
import com.bong.client.combat.DerivedAttrFlags;
import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.store.DerivedAttrsStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

/**
 * Handles {@code derived_attrs_sync} payloads (plan §U6–U7 / §2.4).
 * <pre>{@code
 * {
 *   "flying": true, "flying_qi_remaining": 0.42,
 *   "flying_force_descent_at_ms": 1712999999999,
 *   "phasing": false,
 *   "tribulation_locked": false, "tribulation_stage": "warn",
 *   "throughput_peak_norm": 0.85,
 *   "vortex_fake_skin_layers": 1,
 *   "vortex_ready": true
 * }
 * }</pre>
 */
public final class DerivedAttrsHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        DerivedAttrsStore.State next = new DerivedAttrsStore.State(
            readBoolean(payload, "flying", false),
            (float) readDouble(payload, "flying_qi_remaining", 0d),
            (long) readDouble(payload, "flying_force_descent_at_ms", 0d),
            readBoolean(payload, "phasing", false),
            (long) readDouble(payload, "phasing_until_ms", 0d),
            readBoolean(payload, "tribulation_locked", false),
            readString(payload, "tribulation_stage"),
            (float) readDouble(payload, "throughput_peak_norm", 0d),
            (int) Math.max(0L, Math.round(readDouble(payload, "vortex_fake_skin_layers", 0d))),
            readBoolean(payload, "vortex_ready", false)
        );
        DerivedAttrsStore.replace(next);

        // Keep legacy DerivedAttrFlags inside CombatHudState in sync so existing
        // HUD planners (MiniBodyHudPlanner / EdgeFeedbackHudPlanner) pick it up.
        CombatHudState current = CombatHudStateStore.snapshot();
        DerivedAttrFlags flags = DerivedAttrFlags.of(next.flying(), next.phasing(), next.tribulationLocked());
        CombatHudState merged = CombatHudState.create(
            current.hpPercent(), current.qiPercent(), current.staminaPercent(), flags
        );
        CombatHudStateStore.replace(merged);

        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied derived_attrs_sync (flying=" + next.flying()
                + ", phasing=" + next.phasing()
                + ", tribulation_locked=" + next.tribulationLocked() + ")"
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
        double v = p.getAsDouble();
        return Double.isFinite(v) ? v : fallback;
    }
}
