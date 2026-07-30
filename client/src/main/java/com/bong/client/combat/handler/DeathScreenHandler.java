package com.bong.client.combat.handler;

import com.bong.client.combat.CombatHudStateStore;
import com.bong.client.combat.inspect.WoundLayerBinding;
import com.bong.client.combat.store.DeathStateStore;
import com.bong.client.combat.store.StatusEffectStore;
import com.bong.client.combat.store.WoundsStore;
import com.bong.client.death.DeathCinematicPayloadParser;
import com.bong.client.death.DeathCinematicState;
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
 * Handles {@code death_screen} payloads (plan §U3). Populates
 * {@link DeathStateStore}; the actual full-screen UI is opened lazily by
 * {@link com.bong.client.combat.screen.DeathScreenOpener} when the HUD tick
 * observes {@code visible=true}.
 *
 * <pre>{@code
 * {
 *   "visible": true,
 *   "cause": "pk",
 *   "luck_remaining": 0.37,
 *   "final_words": ["不甘心…", "我要回来"],
 *   "countdown_until_ms": 1712999999999,
 *   "can_reincarnate": true,
 *   "can_terminate": true,
 *   "stage": "tribulation",
 *   "death_number": 4,
 *   "zone_kind": "negative"
 * }
 * }</pre>
 */
public final class DeathScreenHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        boolean visible = readBoolean(payload, "visible", true);
        if (!visible) {
            DeathStateStore.hide();
            return ServerDataDispatch.handled(envelope.type(), "death_screen hidden");
        }
        clearTransientCombatState();
        String cause = readString(payload, "cause");
        float luck = (float) readDouble(payload, "luck_remaining", 0d);
        long deadline = (long) readDouble(payload, "countdown_until_ms", 0d);
        boolean canRein = readBoolean(payload, "can_reincarnate", true);
        boolean canTerm = readBoolean(payload, "can_terminate", false);
        String stage = readString(payload, "stage");
        int deathNumber = (int) readDouble(payload, "death_number", 0d);
        String zoneKind = readString(payload, "zone_kind");

        double yearsLived = 0.0;
        int lifespanCapByRealm = 0;
        double remainingYears = 0.0;
        int deathPenaltyYears = 0;
        double tickRateMultiplier = 0.0;
        boolean windCandle = false;
        JsonObject lifespan = readObject(payload, "lifespan");
        if (lifespan != null) {
            yearsLived = readDouble(lifespan, "years_lived", 0d);
            lifespanCapByRealm = (int) readDouble(lifespan, "cap_by_realm", 0d);
            remainingYears = readDouble(lifespan, "remaining_years", 0d);
            deathPenaltyYears = (int) readDouble(lifespan, "death_penalty_years", 0d);
            tickRateMultiplier = readDouble(lifespan, "tick_rate_multiplier", 0d);
            windCandle = readBoolean(lifespan, "is_wind_candle", false);
        }

        DeathCinematicState cinematic = DeathCinematicPayloadParser.parse(readObject(payload, "cinematic"));

        List<String> finals = new ArrayList<>();
        JsonElement wordsEl = payload.get("final_words");
        if (wordsEl != null && wordsEl.isJsonArray()) {
            JsonArray arr = wordsEl.getAsJsonArray();
            for (JsonElement w : arr) {
                if (w != null && w.isJsonPrimitive() && w.getAsJsonPrimitive().isString()) {
                    finals.add(w.getAsString());
                }
            }
        }

        DeathStateStore.replace(new DeathStateStore.State(
            true, cause, luck, finals, deadline, canRein, canTerm,
            stage, deathNumber, zoneKind, yearsLived, lifespanCapByRealm,
            remainingYears, deathPenaltyYears, tickRateMultiplier, windCandle,
            cinematic
        ));
        return ServerDataDispatch.handled(
            envelope.type(),
            "death_screen visible cause=" + cause + " deadline=" + deadline
        );
    }

    private static void clearTransientCombatState() {
        CombatHudStateStore.clear();
        StatusEffectStore.clear();
        WoundsStore.clear();
        WoundLayerBinding.apply();
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

    private static JsonObject readObject(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        return el != null && el.isJsonObject() ? el.getAsJsonObject() : null;
    }
}
