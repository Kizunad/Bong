package com.bong.client.dying_elder;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

/**
 * plan-dying-elder-v1 P3 — {@code bong:elder_encounter} payload 解析器。
 *
 * <p>解析 server 推送的 {@code ElderEncounterEventV1} JSON payload，
 * 写入 {@link DyingElderEncounterStore}，
 * 供 {@link DyingElderHudPlanner} 渲染垂死大能遭遇 HUD。
 *
 * <p>payload 格式（与 Rust {@code ElderEncounterEventV1} 对齐）：
 * <pre>
 * {
 *   "zone_name":          string,   // 区域名称
 *   "elder_entity_idx":   int,      // 大能实体 idx
 *   "event_kind":         string,   // "appeared" | "dan_received" | "betrayal" | "dead_natural" | "dead_player_kill"
 *   "betray_probability": float,    // 0.0 ~ 1.0
 *   "dan_count":          int,      // 已收丹数
 *   "offered_skill_id":   string,   // 承诺传承功法 id
 *   "server_tick":        long      // server tick 时间戳
 * }
 * </pre>
 *
 * <p><b>守恒红线</b>：本 Handler 只操作 {@link DyingElderEncounterStore}（显示层），
 * <strong>绝不</strong>修改玩家真元、血量或任何 gameplay 数值。
 */
public final class DyingElderEncounterHandler {

    /** bong:elder_encounter channel namespace。 */
    public static final String CHANNEL_NAMESPACE = "bong";

    /** bong:elder_encounter channel path。 */
    public static final String CHANNEL_PATH = "elder_encounter";

    private DyingElderEncounterHandler() {}

    /**
     * 处理来自 {@code bong:elder_encounter} channel 的 JSON payload。
     * 必须在 Minecraft 主线程调用（由 BongNetworkHandler execute() 保证）。
     *
     * @param jsonPayload UTF-8 解码后的 JSON 字符串
     * @return {@code true} = 成功解析并写入 Store；{@code false} = 解析失败（静默跳过）
     */
    public static boolean handle(String jsonPayload) {
        try {
            JsonObject root = JsonParser.parseString(jsonPayload).getAsJsonObject();

            String zoneName = root.has("zone_name")
                ? root.get("zone_name").getAsString()
                : "";
            int elderEntityIdx = root.has("elder_entity_idx")
                ? root.get("elder_entity_idx").getAsInt()
                : 0;
            String eventKind = root.has("event_kind")
                ? root.get("event_kind").getAsString()
                : "appeared";
            double betrayProbability = root.has("betray_probability")
                ? root.get("betray_probability").getAsDouble()
                : 0.0;
            long serverTick = root.has("server_tick")
                ? root.get("server_tick").getAsLong()
                : 0L;

            boolean isDeath = "betrayal".equals(eventKind)
                || "dead_natural".equals(eventKind)
                || "dead_player_kill".equals(eventKind);

            if ("appeared".equals(eventKind)) {
                DyingElderEncounterStore.activate(zoneName, elderEntityIdx, serverTick);
                DyingElderEncounterStore.setBetrayProbability(betrayProbability);
            } else if (!isDeath) {
                // dan_received and any future non-death events
                DyingElderEncounterStore.update(eventKind, serverTick);
                DyingElderEncounterStore.setBetrayProbability(betrayProbability);
            } else {
                // betrayal / dead_natural / dead_player_kill — clears active
                DyingElderEncounterStore.update(eventKind, serverTick);
            }

            return true;
        } catch (Exception e) {
            // 解析失败不崩游戏，静默跳过
            return false;
        }
    }
}
