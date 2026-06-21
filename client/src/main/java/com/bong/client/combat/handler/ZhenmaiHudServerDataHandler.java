package com.bong.client.combat.handler;

import com.bong.client.hud.ZhenmaiHudStateStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

/**
 * 处理 {@code zhenmai_hud} S2C payload，把震脉 5 招的运行时状态投影进
 * {@link ZhenmaiHudStateStore}，供 {@code ZhenmaiHudPlanner} 条件式渲染。
 *
 * <p>payload 形态（mirror dugu_v2 dual-emit；server 侧需在 {@code zhenmai_v2_event_bridge.rs}
 * 追加 S2C dual-emit，见 serverStateNeeded）：
 * <pre>{@code
 * { "v":1, "type":"zhenmai_hud", "skill_id":"parry|neutralize|multipoint|harden|sever_chain",
 *   "meridian_id":"Lung", "contam_removed":2.5, "remaining_points":5,
 *   "damage_reduction":0.65, "k_drain":1.5, "duration_ms":60000, "tick":42 }
 * }</pre>
 *
 * <p>各 skill_id → store 维度映射（per-dimension merge，互不覆盖）：
 * <ul>
 *   <li>{@code parry} → flashParry（瞬态，默认 450ms）</li>
 *   <li>{@code neutralize} → flashNeutralize（contam_removed + meridian，默认 1400ms）</li>
 *   <li>{@code multipoint} → setMultipoint（remaining_points + duration_ms）</li>
 *   <li>{@code harden} → setHarden（damage_reduction + meridian + duration_ms）</li>
 *   <li>{@code sever_chain} → setSever（meridian + k_drain + duration_ms 增幅窗口）</li>
 * </ul>
 *
 * <p>守恒红线：只读 payload，不重算真元/经脉，不改任何其他 store。
 */
public final class ZhenmaiHudServerDataHandler implements ServerDataHandler {

    static final long PARRY_FLASH_MS = 450L;
    static final long NEUTRALIZE_FLASH_MS = 1_400L;
    /** multipoint / harden / sever 缺省持续时间（server 应携 duration_ms 覆盖）。 */
    static final long DEFAULT_DURATION_MS = 2_000L;

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        String type = envelope.type();
        String skillId = readString(payload, "skill_id");
        long now = System.currentTimeMillis();

        switch (skillId) {
            case "parry" -> {
                ZhenmaiHudStateStore.flashParry(now, PARRY_FLASH_MS);
                return ServerDataDispatch.handled(type, "zhenmai parry flash");
            }
            case "neutralize" -> {
                float contamRemoved = (float) readDouble(payload, "contam_removed", 0.0);
                String meridian = readString(payload, "meridian_id");
                ZhenmaiHudStateStore.flashNeutralize(contamRemoved, meridian, now, NEUTRALIZE_FLASH_MS);
                return ServerDataDispatch.handled(type, "zhenmai neutralize meridian=" + meridian + " removed=" + contamRemoved);
            }
            case "multipoint" -> {
                int remaining = (int) Math.round(readDouble(payload, "remaining_points", 0.0));
                long durationMs = readDuration(payload);
                ZhenmaiHudStateStore.setMultipoint(remaining, now, durationMs);
                return ServerDataDispatch.handled(type, "zhenmai multipoint points=" + remaining + " dur=" + durationMs);
            }
            case "harden" -> {
                float reduction = (float) readDouble(payload, "damage_reduction", 0.0);
                String meridian = readString(payload, "meridian_id");
                long durationMs = readDuration(payload);
                ZhenmaiHudStateStore.setHarden(reduction, meridian, now, durationMs);
                return ServerDataDispatch.handled(type, "zhenmai harden meridian=" + meridian + " reduce=" + reduction + " dur=" + durationMs);
            }
            case "sever_chain" -> {
                String meridian = readString(payload, "meridian_id");
                float kDrain = (float) readDouble(payload, "k_drain", 0.0);
                long durationMs = readDuration(payload);
                ZhenmaiHudStateStore.setSever(meridian, kDrain, now, durationMs);
                return ServerDataDispatch.handled(type, "zhenmai sever meridian=" + meridian + " k_drain=" + kDrain + " dur=" + durationMs);
            }
            default -> {
                return ServerDataDispatch.noOp(type, "zhenmai_hud handler: unknown skill_id='" + skillId + "'");
            }
        }
    }

    /** 读 duration_ms，缺省/非法回退 DEFAULT_DURATION_MS。 */
    private static long readDuration(JsonObject obj) {
        JsonElement el = obj.get("duration_ms");
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return DEFAULT_DURATION_MS;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isNumber()) return DEFAULT_DURATION_MS;
        long v = p.getAsLong();
        return v > 0 ? v : DEFAULT_DURATION_MS;
    }

    private static String readString(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return "";
        JsonPrimitive p = el.getAsJsonPrimitive();
        return p.isString() ? p.getAsString() : "";
    }

    private static double readDouble(JsonObject obj, String field, double fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isNumber()) return fallback;
        double v = p.getAsDouble();
        return Double.isFinite(v) ? v : fallback;
    }
}
