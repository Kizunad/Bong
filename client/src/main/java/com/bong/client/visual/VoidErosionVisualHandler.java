package com.bong.client.visual;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

/**
 * plan-combat-skill-feedback-bridges-v1 P3 — {@code bong:void_erosion_visual} payload 解析器。
 *
 * 解析服务端发来的 {@link VoidErosionVisualStore.State} JSON，写入 {@link VoidErosionVisualStore}，
 * 供 {@link VoidErosionHudOverlay} 渲染 HUD 声音扭曲 overlay + 驱动模型 alpha 渐变。
 *
 * <p>payload 格式（对应 Rust {@code VoidErosionVisualSyncPayloadV1}）：
 * <pre>
 * {
 *   "entity_id":              string,
 *   "stage":                  int (0-4),
 *   "cumulative_erosion":     number,
 *   "ambient_active":         bool,
 *   "model_alpha":            float (0.0~1.0),
 *   "sound_distortion_active": bool,
 *   "server_tick":            long
 * }
 * </pre>
 */
public final class VoidErosionVisualHandler {

    private VoidErosionVisualHandler() {}

    /**
     * 处理来自 {@code bong:void_erosion_visual} channel 的 JSON payload。
     * 必须在 Minecraft 主线程调用（由 BongNetworkHandler execute() 保证）。
     *
     * @param jsonPayload UTF-8 解码后的 JSON 字符串
     */
    public static void handle(String jsonPayload) {
        try {
            JsonObject root = JsonParser.parseString(jsonPayload).getAsJsonObject();

            String entityId = root.has("entity_id")
                    ? root.get("entity_id").getAsString()
                    : "unknown";
            int stage = root.has("stage")
                    ? Math.max(0, Math.min(4, root.get("stage").getAsInt()))
                    : 0;
            double cumulativeErosion = root.has("cumulative_erosion")
                    ? root.get("cumulative_erosion").getAsDouble()
                    : 0.0;
            boolean ambientActive = root.has("ambient_active")
                    && root.get("ambient_active").getAsBoolean();
            float modelAlpha = root.has("model_alpha")
                    ? Math.max(0.0f, Math.min(1.0f, root.get("model_alpha").getAsFloat()))
                    : 1.0f;
            boolean soundDistortionActive = root.has("sound_distortion_active")
                    && root.get("sound_distortion_active").getAsBoolean();

            VoidErosionVisualStore.replace(
                    entityId,
                    stage,
                    cumulativeErosion,
                    ambientActive,
                    modelAlpha,
                    soundDistortionActive
            );
        } catch (Exception e) {
            // 解析失败不崩游戏，静默跳过
        }
    }
}
