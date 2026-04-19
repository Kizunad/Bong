package com.bong.client.network.alchemy;

import com.bong.client.alchemy.state.AlchemyOutcomeForecastStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

/** plan-alchemy-v1 §4 — `alchemy_outcome_forecast` payload → {@link AlchemyOutcomeForecastStore}. */
public final class AlchemyOutcomeForecastHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        try {
            float perfect = (float) readDouble(p, "perfect_pct");
            float good = (float) readDouble(p, "good_pct");
            float flawed = (float) readDouble(p, "flawed_pct");
            float waste = (float) readDouble(p, "waste_pct");
            float explode = (float) readDouble(p, "explode_pct");
            String pn = readString(p, "perfect_note");
            String gn = readString(p, "good_note");
            String fn = readString(p, "flawed_note");
            AlchemyOutcomeForecastStore.replace(new AlchemyOutcomeForecastStore.Snapshot(
                perfect, good, flawed, waste, explode, pn, gn, fn));
            return ServerDataDispatch.handled(envelope.type(),
                "Applied alchemy_outcome_forecast snapshot");
        } catch (RuntimeException e) {
            return ServerDataDispatch.noOp(envelope.type(),
                "alchemy_outcome_forecast payload malformed: " + e.getMessage());
        }
    }

    private static double readDouble(JsonObject obj, String key) {
        if (!obj.has(key) || obj.get(key).isJsonNull()) return 0.0;
        JsonElement el = obj.get(key);
        if (!el.isJsonPrimitive() || !el.getAsJsonPrimitive().isNumber()) return 0.0;
        double v = el.getAsDouble();
        return Double.isFinite(v) ? v : 0.0;
    }

    private static String readString(JsonObject obj, String key) {
        if (!obj.has(key) || obj.get(key).isJsonNull()) return "";
        JsonElement el = obj.get(key);
        return el.isJsonPrimitive() ? el.getAsString() : "";
    }
}
