package com.bong.client.network.lingtian;

import com.bong.client.lingtian.state.LingtianSessionStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

/**
 * plan-lingtian-v1 §4 — 解析 {@code lingtian_session} payload → {@link LingtianSessionStore}.
 *
 * <p>Wire format（proto {@code LingtianSessionData}，{@code envelope.proto:1351-1362}，
 * 生产链经 {@link com.bong.client.network.ProtoServerDataBridge} 桥出）：</p>
 * <pre>
 * {
 *   "type": "lingtian_session",
 *   "v": 1,
 *   "active": true,
 *   "kind": "till" | "renew" | "planting" | "harvest" | "replenish" | "drain_qi",
 *   "pos_x": 12, "pos_y": 64, "pos_z": -7,
 *   "elapsed_ticks": 12,
 *   "target_ticks": 40,
 *   "plant_id": "ci_she_hao",  // 仅 planting / harvest
 *   "source": "pill_residue_failed_pill",
 *   "dye_contamination": 0.31,
 *   "dye_contamination_warning": true
 * }
 * </pre>
 *
 * <p>plan-wire-format-bridge-v1 P2/RC3 — 旧实现用 {@code readIntArray3(p, "pos")} 读**数组**
 * 形状，但 proto {@code pos_x/pos_y/pos_z} 是拆开的 flat int32 三字段（{@code envelope.proto:1353-1355}），
 * JSON 里从无 {@code "pos"} 数组字段 → 恒静默 fallback {@code {0,0,0}}（比 noOp 更隐蔽，因为
 * dispatch 仍标 handled）。改读 flat 三字段（比照 {@code ExtractServerDataHandler#readFlatVec3}
 * 的 double 版本，此处为 int 版本）。</p>
 */
public final class LingtianSessionHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        try {
            boolean active = p.has("active") && p.get("active").getAsBoolean();
            String kindStr = readString(p, "kind", "till");
            int[] pos = readFlatVec3(p, "pos");
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

    /**
     * 读 proto flat 三标量坐标（{@code <prefix>_x/_y/_z}，均为 int32）。任一字段缺失或非数字时
     * 该分量 fallback 0（与旧数组读法的 fallback 行为一致，避免因单字段缺失整体丢弃 snapshot）。
     */
    private static int[] readFlatVec3(JsonObject obj, String prefix) {
        return new int[]{
            readInt(obj, prefix + "_x", 0),
            readInt(obj, prefix + "_y", 0),
            readInt(obj, prefix + "_z", 0)
        };
    }
}
