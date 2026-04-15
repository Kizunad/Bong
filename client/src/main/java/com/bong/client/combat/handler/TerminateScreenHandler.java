package com.bong.client.combat.handler;

import com.bong.client.combat.store.TerminateStateStore;
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
            readString(payload, "archetype_suggestion")
        ));
        return ServerDataDispatch.handled(envelope.type(), "terminate_screen visible");
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
