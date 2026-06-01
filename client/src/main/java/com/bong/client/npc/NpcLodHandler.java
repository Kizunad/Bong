package com.bong.client.npc;

import com.bong.client.network.ServerDataEnvelope;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

/**
 * {@code bong:npc_lod} CustomPayload 解析器（plan-offscreen-war-v1 §P7 PR-11）。
 *
 * <p>消费 server 每 80 tick（约 4 秒）推送的低 LOD NPC 渲染数据，写入 {@link NpcLodStore}。
 *
 * <p><b>守恒红线</b>：此 handler 不触碰任何 qi / ledger 字段；
 * 解析失败静默返回 {@code false}，不抛异常。
 *
 * <p>payload 格式（对应 {@code NpcLodS2c} Rust struct）：
 * <pre>
 * {
 *   "v": 1,
 *   "type": "npc_lod",
 *   "entity_id": 42,
 *   "archetype": "rogue",
 *   "realm": "引气",
 *   "hp_ratio": 0.75,
 *   "x": 100.5, "y": 64.0, "z": -200.0
 * }
 * </pre>
 */
public final class NpcLodHandler {
    public static final String CHANNEL_NAMESPACE = "bong";
    public static final String CHANNEL_PATH = "npc_lod";
    public static final int VERSION = 1;

    private NpcLodHandler() {
    }

    /**
     * 处理一条 {@code bong:npc_lod} payload。
     *
     * @param jsonPayload      UTF-8 解码后的 JSON 字符串
     * @param payloadSizeBytes payload 原始字节数（用于超限检查）
     * @return {@code true} 解析并写入成功；{@code false} 任意验证失败
     */
    public static boolean handle(String jsonPayload, int payloadSizeBytes) {
        if (jsonPayload == null || payloadSizeBytes < 0
                || payloadSizeBytes > ServerDataEnvelope.MAX_PAYLOAD_BYTES) {
            return false;
        }
        JsonObject root;
        try {
            root = JsonParser.parseString(jsonPayload).getAsJsonObject();
        } catch (RuntimeException exception) {
            return false;
        }

        // 版本 + type 门禁
        if (intField(root, "v", -1) != VERSION) {
            return false;
        }
        if (!"npc_lod".equals(stringField(root, "type", ""))) {
            return false;
        }

        int entityId = intField(root, "entity_id", Integer.MIN_VALUE);
        if (entityId < 0 || entityId == Integer.MIN_VALUE) {
            return false;
        }

        NpcLodStore.upsert(new NpcLodSnapshot(
            entityId,
            stringField(root, "archetype", "unknown"),
            stringField(root, "realm", "醒灵"),
            (float) doubleField(root, "hp_ratio", 1.0),
            doubleField(root, "x", 0.0),
            doubleField(root, "y", 0.0),
            doubleField(root, "z", 0.0)
        ));
        return true;
    }

    // ── field helpers ────────────────────────────────────────────────────────

    private static int intField(JsonObject root, String fieldName, int fallback) {
        if (!root.has(fieldName) || root.get(fieldName).isJsonNull()) {
            return fallback;
        }
        try {
            return root.get(fieldName).getAsInt();
        } catch (RuntimeException exception) {
            return fallback;
        }
    }

    private static String stringField(JsonObject root, String fieldName, String fallback) {
        if (!root.has(fieldName) || root.get(fieldName).isJsonNull()) {
            return fallback;
        }
        try {
            String value = root.get(fieldName).getAsString();
            return (value == null || value.isBlank()) ? fallback : value.trim();
        } catch (RuntimeException exception) {
            return fallback;
        }
    }

    private static double doubleField(JsonObject root, String fieldName, double fallback) {
        if (!root.has(fieldName) || root.get(fieldName).isJsonNull()) {
            return fallback;
        }
        try {
            double value = root.get(fieldName).getAsDouble();
            return Double.isFinite(value) ? value : fallback;
        } catch (RuntimeException exception) {
            return fallback;
        }
    }
}
