package com.bong.client.combat.handler;

import com.bong.client.combat.store.TerminateStateStore;
import com.bong.client.combat.store.TerminationSummary;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

/**
 * Handles {@code terminate_screen} payloads (plan §U4).
 * <pre>{@code
 * { "visible": true,
 *   "final_words": "...",
 *   "epilogue": "...",
 *   "archetype_suggestion": "游侠" }
 * }</pre>
 */
public final class TerminateScreenHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        boolean visible = readBoolean(payload, "visible", true);
        if (!visible) {
            TerminateStateStore.hide();
            return ServerDataDispatch.handled(envelope.type(), "terminate_screen hidden");
        }
        TerminateStateStore.replace(new TerminateStateStore.State(
            true,
            readString(payload, "final_words"),
            readString(payload, "epilogue"),
            readString(payload, "archetype_suggestion"),
            readSummary(payload)
        ));
        return ServerDataDispatch.handled(envelope.type(), "terminate_screen visible");
    }

    private static TerminationSummary readSummary(JsonObject payload) {
        if (!payload.has("summary") || !payload.get("summary").isJsonObject()) {
            return TerminationSummary.EMPTY;
        }
        JsonObject summary = payload.getAsJsonObject("summary");
        Integer deaths = readCount(summary, "death_count");
        return new TerminationSummary(readString(summary, "character_name"), readString(summary, "realm"),
            deaths == null ? 0 : deaths, readNumber(summary, "years_lived"), readNumber(summary, "qi_max"),
            readNumber(summary, "health_max"), readCount(summary, "meridians_open"), readCount(summary, "techniques_learned"));
    }

    private static Double readNumber(JsonObject object, String field) {
        JsonElement value = object.get(field);
        if (value == null || !value.isJsonPrimitive() || !value.getAsJsonPrimitive().isNumber()) return null;
        double number = value.getAsDouble();
        return Double.isFinite(number) && number >= 0 ? number : null;
    }

    private static Integer readCount(JsonObject object, String field) {
        Double number = readNumber(object, field);
        return number == null || number > Integer.MAX_VALUE ? null : number.intValue();
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
}
