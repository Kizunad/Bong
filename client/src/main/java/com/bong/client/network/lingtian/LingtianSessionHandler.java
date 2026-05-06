package com.bong.client.network.lingtian;

import com.bong.client.lingtian.state.LingtianSessionStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

/**
 * plan-lingtian-v1 §4 — 解析 {@code lingtian_session} payload → {@link LingtianSessionStore}.
 *
 * <p>Wire format（与 server schema/lingtian.rs 1:1）：</p>
 * <pre>
 * {
 *   "type": "lingtian_session",
 *   "v": 1,
 *   "active": true,
 *   "kind": "till" | "renew" | "planting" | "harvest" | "replenish" | "drain_qi",
 *   "pos": [x, y, z],
 *   "elapsed_ticks": 12,
 *   "target_ticks": 40,
 *   "plant_id": "ci_she_hao",  // 仅 planting / harvest
 *   "source": "pill_residue_failed_pill",
 *   "dye_contamination": 0.31,
 *   "dye_contamination_warning": true
 * }
 * </pre>
 */
public final class LingtianSessionHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        try {
            boolean active = p.has("active") && p.get("active").getAsBoolean();
            String kindStr = readString(p, "kind", "till");
            int[] pos = readIntArray3(p, "pos");
            int elapsed = readInt(p, "elapsed_ticks", 0);
            int target = readInt(p, "target_ticks", 0);
            String plantId = p.has("plant_id") && p.get("plant_id").isJsonPrimitive()
                ? p.get("plant_id").getAsString()
                : null;
            String source = p.has("source") && p.get("source").isJsonPrimitive()
                ? p.get("source").getAsString()
                : null;
            float dyeContamination = readFloat(p, "dye_contamination", 0.0f);
            boolean dyeWarning = p.has("dye_contamination_warning")
                && p.get("dye_contamination_warning").isJsonPrimitive()
                && p.get("dye_contamination_warning").getAsBoolean();

            LingtianSessionStore.replace(new LingtianSessionStore.Snapshot(
                active,
                LingtianSessionStore.Kind.fromWire(kindStr),
                pos[0], pos[1], pos[2],
                elapsed, target,
                plantId,
                source,
                dyeContamination,
                dyeWarning
            ));
            return ServerDataDispatch.handled(envelope.type(),
                "Applied lingtian_session snapshot (active=" + active + ", kind=" + kindStr + ", "
                    + elapsed + "/" + target + ")");
        } catch (RuntimeException e) {
            return ServerDataDispatch.noOp(envelope.type(),
                "lingtian_session payload malformed: " + e.getMessage());
        }
    }

    private static int readInt(JsonObject obj, String key, int fallback) {
        if (!obj.has(key) || obj.get(key).isJsonNull()) return fallback;
        JsonElement el = obj.get(key);
        if (!el.isJsonPrimitive() || !el.getAsJsonPrimitive().isNumber()) return fallback;
        return el.getAsInt();
    }

    private static String readString(JsonObject obj, String key, String fallback) {
        if (!obj.has(key) || obj.get(key).isJsonNull()) return fallback;
        JsonElement el = obj.get(key);
        return el.isJsonPrimitive() ? el.getAsString() : fallback;
    }

    private static float readFloat(JsonObject obj, String key, float fallback) {
        if (!obj.has(key) || obj.get(key).isJsonNull()) return fallback;
        JsonElement el = obj.get(key);
        if (!el.isJsonPrimitive() || !el.getAsJsonPrimitive().isNumber()) return fallback;
        return el.getAsFloat();
    }

    private static int[] readIntArray3(JsonObject obj, String key) {
        int[] out = new int[]{0, 0, 0};
        if (!obj.has(key) || !obj.get(key).isJsonArray()) return out;
        JsonArray arr = obj.getAsJsonArray(key);
        for (int i = 0; i < 3 && i < arr.size(); i++) {
            JsonElement el = arr.get(i);
            if (el.isJsonPrimitive() && el.getAsJsonPrimitive().isNumber()) {
                out[i] = el.getAsInt();
            }
        }
        return out;
    }
}
