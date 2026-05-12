package com.bong.client.network.alchemy;

import com.bong.client.alchemy.state.AlchemySessionStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import java.util.ArrayList;
import java.util.List;

/** plan-alchemy-v1 §4 — `alchemy_session` payload → {@link AlchemySessionStore}. */
public final class AlchemySessionHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        try {
            String recipeId = readString(p, "recipe_id", "");
            boolean active = p.has("active") && p.get("active").isJsonPrimitive() && p.get("active").getAsBoolean();
            int elapsed = readInt(p, "elapsed_ticks", 0);
            int target = readInt(p, "target_ticks", 0);
            float tempCur = (float) readDouble(p, "temp_current", 0.0);
            float tempTgt = (float) readDouble(p, "temp_target", 0.0);
            float tempBand = (float) readDouble(p, "temp_band", 0.0);
            double qiInj = readDouble(p, "qi_injected", 0.0);
            double qiTgt = readDouble(p, "qi_target", 0.0);
            String status = readString(p, "status_label", "");
            List<AlchemySessionStore.StageHint> stages = new ArrayList<>();
            JsonArray stagesArr = p.has("stages") && p.get("stages").isJsonArray()
                ? p.getAsJsonArray("stages") : null;
            if (stagesArr != null) {
                for (JsonElement el : stagesArr) {
                    if (!el.isJsonObject()) continue;
                    JsonObject s = el.getAsJsonObject();
                    stages.add(new AlchemySessionStore.StageHint(
                        readInt(s, "at_tick", 0),
                        readInt(s, "window", 0),
                        readString(s, "summary", ""),
                        s.has("completed") && s.get("completed").getAsBoolean(),
                        s.has("missed") && s.get("missed").getAsBoolean()
                    ));
                }
            }
            List<String> log = new ArrayList<>();
            JsonArray ints = p.has("interventions_recent") && p.get("interventions_recent").isJsonArray()
                ? p.getAsJsonArray("interventions_recent") : null;
            if (ints != null) {
                for (JsonElement el : ints) if (el.isJsonPrimitive()) log.add(el.getAsString());
            }
            AlchemySessionStore.replace(new AlchemySessionStore.Snapshot(
                recipeId, active, elapsed, target, tempCur, tempTgt, tempBand, qiInj, qiTgt,
                status, List.copyOf(stages), List.copyOf(log)));
            return ServerDataDispatch.handled(envelope.type(),
                "Applied alchemy_session snapshot (elapsed=" + elapsed + "/" + target + ")");
        } catch (RuntimeException e) {
            return ServerDataDispatch.noOp(envelope.type(),
                "alchemy_session payload malformed: " + e.getMessage());
        }
    }

    private static String readString(JsonObject obj, String key, String fallback) {
        if (!obj.has(key) || obj.get(key).isJsonNull()) return fallback;
        JsonElement el = obj.get(key);
        return el.isJsonPrimitive() ? el.getAsString() : fallback;
    }

    private static int readInt(JsonObject obj, String key, int fallback) {
        if (!obj.has(key) || obj.get(key).isJsonNull()) return fallback;
        JsonElement el = obj.get(key);
        if (!el.isJsonPrimitive() || !el.getAsJsonPrimitive().isNumber()) return fallback;
        return el.getAsInt();
    }

    private static double readDouble(JsonObject obj, String key, double fallback) {
        if (!obj.has(key) || obj.get(key).isJsonNull()) return fallback;
        JsonElement el = obj.get(key);
        if (!el.isJsonPrimitive() || !el.getAsJsonPrimitive().isNumber()) return fallback;
        double v = el.getAsDouble();
        return Double.isFinite(v) ? v : fallback;
    }
}
