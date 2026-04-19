package com.bong.client.network.alchemy;

import com.bong.client.alchemy.state.ContaminationWarningStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

/**
 * plan-alchemy-v1 §4 — `alchemy_contamination` payload → {@link ContaminationWarningStore}.
 *
 * <p>Server 推送的 {@code levels[]} 含 N 条 (color, current, max, ok)；当前 client store
 * 仅暴露 Mellow / Violent 两条快捷字段，其他色暂时丢弃（后续若 HUD 接入更多色再扩展）。
 */
public final class AlchemyContaminationHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        try {
            float mellowCur = 0, mellowMax = 0;
            boolean mellowOk = true;
            float violentCur = 0, violentMax = 0;
            boolean violentOk = true;
            JsonArray levels = p.has("levels") && p.get("levels").isJsonArray()
                ? p.getAsJsonArray("levels") : null;
            if (levels != null) {
                for (JsonElement el : levels) {
                    if (!el.isJsonObject()) continue;
                    JsonObject lvl = el.getAsJsonObject();
                    String color = readString(lvl, "color", "");
                    float cur = (float) readDouble(lvl, "current");
                    float max = (float) readDouble(lvl, "max");
                    boolean ok = lvl.has("ok") && lvl.get("ok").getAsBoolean();
                    if ("Mellow".equals(color)) { mellowCur = cur; mellowMax = max; mellowOk = ok; }
                    else if ("Violent".equals(color)) { violentCur = cur; violentMax = max; violentOk = ok; }
                }
            }
            String note = readString(p, "metabolism_note", "");
            ContaminationWarningStore.replace(new ContaminationWarningStore.Snapshot(
                mellowCur, mellowMax, mellowOk,
                violentCur, violentMax, violentOk,
                note));
            return ServerDataDispatch.handled(envelope.type(),
                "Applied alchemy_contamination snapshot");
        } catch (RuntimeException e) {
            return ServerDataDispatch.noOp(envelope.type(),
                "alchemy_contamination payload malformed: " + e.getMessage());
        }
    }

    private static double readDouble(JsonObject obj, String key) {
        if (!obj.has(key) || obj.get(key).isJsonNull()) return 0.0;
        JsonElement el = obj.get(key);
        if (!el.isJsonPrimitive() || !el.getAsJsonPrimitive().isNumber()) return 0.0;
        double v = el.getAsDouble();
        return Double.isFinite(v) ? v : 0.0;
    }

    private static String readString(JsonObject obj, String key, String fallback) {
        if (!obj.has(key) || obj.get(key).isJsonNull()) return fallback;
        JsonElement el = obj.get(key);
        return el.isJsonPrimitive() ? el.getAsString() : fallback;
    }
}
