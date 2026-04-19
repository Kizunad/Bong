package com.bong.client.network.alchemy;

import com.bong.client.alchemy.state.AlchemyAttemptHistoryStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

/** plan-alchemy-v1 §4 — `alchemy_outcome_resolved` payload → {@link AlchemyAttemptHistoryStore}. */
public final class AlchemyOutcomeResolvedHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        try {
            String bucket = readString(p, "bucket", "");
            String recipeId = readString(p, "recipe_id", "");
            String pill = readString(p, "pill", "");
            String color = readString(p, "toxin_color", "");
            String tag = readString(p, "side_effect_tag", "");
            boolean flawed = p.has("flawed_path") && p.get("flawed_path").getAsBoolean();
            AlchemyAttemptHistoryStore.append(new AlchemyAttemptHistoryStore.Entry(
                bucket, recipeId, pill, color, tag, flawed));
            return ServerDataDispatch.handled(envelope.type(),
                "Appended alchemy_outcome_resolved (bucket=" + bucket + ") to history");
        } catch (RuntimeException e) {
            return ServerDataDispatch.noOp(envelope.type(),
                "alchemy_outcome_resolved payload malformed: " + e.getMessage());
        }
    }

    private static String readString(JsonObject obj, String key, String fallback) {
        if (!obj.has(key) || obj.get(key).isJsonNull()) return fallback;
        JsonElement el = obj.get(key);
        return el.isJsonPrimitive() ? el.getAsString() : fallback;
    }
}
