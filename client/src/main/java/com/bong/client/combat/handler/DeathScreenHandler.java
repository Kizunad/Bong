package com.bong.client.combat.handler;

import com.bong.client.combat.store.DeathStateStore;
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
 *   "can_terminate": true
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
        String cause = readString(payload, "cause");
        float luck = (float) readDouble(payload, "luck_remaining", 0d);
        long deadline = (long) readDouble(payload, "countdown_until_ms", 0d);
        boolean canRein = readBoolean(payload, "can_reincarnate", true);
        boolean canTerm = readBoolean(payload, "can_terminate", false);

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
            true, cause, luck, finals, deadline, canRein, canTerm
        ));
        return ServerDataDispatch.handled(
            envelope.type(),
            "death_screen visible cause=" + cause + " deadline=" + deadline
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
