package com.bong.client.combat.handler;

import com.bong.client.combat.store.HalfStepRechallengeStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.JsonPrimitive;

/**
 * Handles {@code bong:halfstep_rechallenge} payloads (plan-halfstep-rechallenge-integration-v1 P0).
 *
 * <p>Wire protocol: server sends bare {@code HalfStepRechallengeV1} JSON via a dedicated
 * {@code bong:halfstep_rechallenge} CustomPayload channel (not wrapped in {@code ServerDataV1}
 * envelope, not routed through {@code ServerDataRouter}).
 * {@link com.bong.client.BongNetworkHandler#register()} registers the channel listener,
 * which calls {@link #handle(String)} with the raw JSON string.
 *
 * <p>The {@link ServerDataHandler} interface method {@link #handle(ServerDataEnvelope)} is
 * retained for backward compatibility with existing unit tests that drive the handler directly
 * via a parsed {@link ServerDataEnvelope}.
 *
 * <p>Last-write-wins. {@code active=false} → {@link HalfStepRechallengeStore#clear()}.
 *
 * <pre>{@code
 * // Bare JSON sent on bong:halfstep_rechallenge channel (no ServerDataV1 wrapper):
 * { "active": true,
 *   "char_id": "offline:Azure",
 *   "rechallenge_window_until": 4032000,
 *   "at_tick": 10000 }
 * }</pre>
 */
public final class HalfStepRechallengeHandler implements ServerDataHandler {

    /**
     * Entry point for the dedicated {@code bong:halfstep_rechallenge} channel.
     * Parses the raw JSON string (bare {@code HalfStepRechallengeV1}, no envelope) and
     * writes the result to {@link HalfStepRechallengeStore}.
     *
     * @param json raw UTF-8 JSON string received from the server
     */
    public static void handle(String json) {
        if (json == null || json.isBlank()) {
            com.bong.client.BongClient.LOGGER.warn(
                "[bong][halfstep_rechallenge] received blank payload, ignoring");
            return;
        }
        try {
            JsonObject root = JsonParser.parseString(json).getAsJsonObject();
            boolean active = readBoolean(root, "active", true);
            if (!active) {
                HalfStepRechallengeStore.clear();
                com.bong.client.BongClient.LOGGER.info(
                    "[bong][halfstep_rechallenge] HIDE received — store cleared");
                return;
            }
            String charId = readString(root, "char_id");
            long windowUntilTick = readLong(root, "rechallenge_window_until", 0L);
            long atTick = readLong(root, "at_tick", 0L);
            long nowMs = System.currentTimeMillis();
            HalfStepRechallengeStore.replace(new HalfStepRechallengeStore.State(
                true, charId, windowUntilTick, atTick, nowMs
            ));
            com.bong.client.BongClient.LOGGER.info(
                "[bong][halfstep_rechallenge] TRIGGER received — char={} window={} at_tick={}",
                charId, windowUntilTick, atTick);
        } catch (Exception ex) {
            com.bong.client.BongClient.LOGGER.error(
                "[bong][halfstep_rechallenge] failed to parse payload: {}", ex.getMessage());
        }
    }

    /**
     * {@link ServerDataHandler} interface method — retained to allow existing unit tests
     * that drive this handler directly via a {@link ServerDataEnvelope} to continue working.
     * In production, payloads arrive through the dedicated channel and {@link #handle(String)}
     * is called instead.
     */
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        boolean active = readBoolean(payload, "active", true);
        if (!active) {
            HalfStepRechallengeStore.clear();
            return ServerDataDispatch.handled(envelope.type(), "halfstep rechallenge cleared");
        }
        String charId = readString(payload, "char_id");
        long windowUntilTick = readLong(payload, "rechallenge_window_until", 0L);
        long atTick = readLong(payload, "at_tick", 0L);
        long nowMs = System.currentTimeMillis();
        HalfStepRechallengeStore.replace(new HalfStepRechallengeStore.State(
            true, charId, windowUntilTick, atTick, nowMs
        ));
        return ServerDataDispatch.handled(envelope.type(), "halfstep rechallenge active");
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

    private static long readLong(JsonObject obj, String field, long fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isNumber()) return fallback;
        double v = p.getAsDouble();
        return Double.isFinite(v) ? (long) v : fallback;
    }
}
