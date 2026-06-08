package com.bong.client.era;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

/**
 * plan-era-state-v1 P3 — 时代天象 S2C CustomPayload 客户端处理器。
 *
 * <p>接收 {@code bong:era_ambiance} channel 的 JSON payload，解析时代天象参数并存入
 * {@link EraAmbianceState}。渲染层（{@link ZoneAtmosphereRenderer} / {@link ZoneAtmospherePlanner}）
 * 通过 {@link EraAmbianceState#current()} 读取当前时代叠加层。
 *
 * <p>payload JSON 结构（server 端 {@code EraAmbianceS2c} 序列化）：
 * <pre>{@code
 * {
 *   "v": 1,
 *   "era_type": "calamity" | "change" | "deduction" | "unknown",
 *   "sky_tint_hex": "#RRGGBB",
 *   "fog_density_delta": 0.15,
 *   "ambient_sound_id": "ambient.weather.thunder",
 *   "transition_ticks": 1200
 * }
 * }</pre>
 *
 * <p>Realm gate 由 server 强制执行，client 收到即渲染（不做 realm 过滤）。
 *
 * <h3>线性插值（渐变）</h3>
 * <p>渐变通过 {@link EraAmbianceState} 的 {@code interpolateSkyTint} 和
 * {@code interpolateFogDensityDelta} 方法实现，基于 {@code transition_ticks}（1200 tick = 60s）
 * 在 game tick 间线性插值。调用方在每帧 tick 推进计数器。
 *
 * <h3>使用模式</h3>
 * <pre>{@code
 * // BongNetworkHandler.java 接收时：
 * EraAmbianceHandler.handle(jsonPayload);
 *
 * // ZoneAtmospherePlanner / ZoneAtmosphereRenderer 调用：
 * EraAmbiancePayload era = EraAmbianceState.current();
 * if (era != null && era.transitionProgress() < 1.0) { ... }
 * }</pre>
 */
public final class EraAmbianceHandler {

    private EraAmbianceHandler() {
    }

    /**
     * 处理 {@code bong:era_ambiance} payload。
     *
     * <p>解析 JSON，校验版本（v=1），构造 {@link EraAmbiancePayload} 存入 {@link EraAmbianceState}。
     * 解析失败时静默记录日志，不抛异常（保持与其他 handler 一致的容错风格）。
     *
     * @param json server 发来的 UTF-8 JSON 字符串
     * @return 解析结果；{@code null} = 解析失败（已记录到日志）
     */
    public static EraAmbiancePayload handle(String json) {
        if (json == null || json.isBlank()) {
            com.bong.client.BongClient.LOGGER.warn(
                "[bong][era_ambiance] received blank payload, ignoring");
            return null;
        }
        try {
            JsonObject root = JsonParser.parseString(json).getAsJsonObject();
            int v = readInt(root, "v", -1);
            if (v != 1) {
                com.bong.client.BongClient.LOGGER.warn(
                    "[bong][era_ambiance] unknown protocol version v={}, ignoring", v);
                return null;
            }
            String eraType = readString(root, "era_type", "unknown");
            String skyTintHex = readString(root, "sky_tint_hex", "#000000");
            double fogDensityDelta = readDouble(root, "fog_density_delta", 0.0);
            String ambientSoundId = readString(root, "ambient_sound_id", "");
            int transitionTicks = readInt(root, "transition_ticks", 1200);

            int skyTintRgb = parseHexColorSafe(skyTintHex);

            EraAmbiancePayload payload = new EraAmbiancePayload(
                eraType, skyTintRgb, fogDensityDelta, ambientSoundId, transitionTicks
            );
            EraAmbianceState.accept(payload);

            com.bong.client.BongClient.LOGGER.info(
                "[bong][era_ambiance] accepted era={} fog_delta={} sky=#{} sound={} transition={}t",
                eraType, fogDensityDelta, String.format("%06X", skyTintRgb),
                ambientSoundId.isEmpty() ? "(none)" : ambientSoundId,
                transitionTicks
            );
            return payload;
        } catch (Exception ex) {
            com.bong.client.BongClient.LOGGER.error(
                "[bong][era_ambiance] failed to parse payload: {}", ex.getMessage());
            return null;
        }
    }

    // ── JSON 读取辅助 ──────────────────────────────────────────────────────────

    private static String readString(JsonObject obj, String field, String fallback) {
        var element = obj.get(field);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return fallback;
        }
        var prim = element.getAsJsonPrimitive();
        return prim.isString() ? prim.getAsString() : fallback;
    }

    private static double readDouble(JsonObject obj, String field, double fallback) {
        var element = obj.get(field);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return fallback;
        }
        var prim = element.getAsJsonPrimitive();
        return prim.isNumber() ? prim.getAsDouble() : fallback;
    }

    private static int readInt(JsonObject obj, String field, int fallback) {
        var element = obj.get(field);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) {
            return fallback;
        }
        var prim = element.getAsJsonPrimitive();
        return prim.isNumber() ? prim.getAsInt() : fallback;
    }

    /**
     * 解析 {@code #RRGGBB} hex 颜色字符串为 RGB int。
     * 无法解析时返回 {@code 0x000000}（黑色 = baseline，不修改天空色调）。
     *
     * @param hex 颜色字符串，如 {@code "#4A1A1A"} 或 {@code "4A1A1A"}
     * @return RGB int (0x000000..0xFFFFFF)
     */
    static int parseHexColorSafe(String hex) {
        if (hex == null) {
            return 0;
        }
        String text = hex.trim();
        if (text.startsWith("#")) {
            text = text.substring(1);
        }
        // 允许 AARRGGBB 8 位：去掉 alpha
        if (text.length() == 8) {
            text = text.substring(2);
        }
        if (text.length() != 6) {
            return 0;
        }
        try {
            return Integer.parseInt(text, 16) & 0x00FFFFFF;
        } catch (NumberFormatException ex) {
            return 0;
        }
    }
}
