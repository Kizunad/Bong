package com.bong.client.fauna;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

/**
 * plan-fauna-stitched-beast-v1 P3 — {@code bong:core_absorption_hallucination} payload 解析器。
 *
 * <p>解析 server 推送的 JSON payload，写入 {@link HallucinationLayerStore}，
 * 供 {@link HallucinationHudOverlay} 渲染感知幻觉 HUD。
 *
 * <p>payload 格式：
 * <pre>
 * {
 *   "duration_ticks": int,    // 幻觉持续 tick 数；0 = 立即取消
 *   "cancel":         bool    // true = 强制取消（断线/到期）；false = 激活（默认）
 * }
 * </pre>
 *
 * <p><b>守恒红线</b>：本 Handler 只操作 {@link HallucinationLayerStore}（显示层），
 * <strong>绝不</strong>修改玩家实际 HP / qi_current 或任何 gameplay 数据。
 */
public final class HallucinationLayerHandler {

    /** bong:core_absorption_hallucination channel namespace。 */
    public static final String CHANNEL_NAMESPACE = "bong";

    /** bong:core_absorption_hallucination channel path。 */
    public static final String CHANNEL_PATH = "core_absorption_hallucination";

    private HallucinationLayerHandler() {}

    /**
     * 处理来自 {@code bong:core_absorption_hallucination} channel 的 JSON payload。
     * 必须在 Minecraft 主线程调用（由 BongNetworkHandler execute() 保证）。
     *
     * @param jsonPayload UTF-8 解码后的 JSON 字符串
     */
    public static void handle(String jsonPayload) {
        try {
            JsonObject root = JsonParser.parseString(jsonPayload).getAsJsonObject();

            // duration_ticks：0 = 立即取消，正值 = 激活
            int durationTicks = root.has("duration_ticks")
                    ? root.get("duration_ticks").getAsInt()
                    : 0;

            // cancel 字段（true = 强制取消）
            boolean cancel = root.has("cancel") && root.get("cancel").getAsBoolean();

            if (cancel || durationTicks <= 0) {
                HallucinationLayerStore.cancel();
            } else {
                HallucinationLayerStore.activate(durationTicks);
            }
        } catch (Exception e) {
            // 解析失败不崩游戏，静默跳过（bong:core_absorption_hallucination 非关键路径）
        }
    }
}
