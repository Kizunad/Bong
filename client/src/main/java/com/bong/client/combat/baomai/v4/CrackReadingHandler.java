package com.bong.client.combat.baomai.v4;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import java.util.ArrayList;
import java.util.List;

/**
 * plan-combat-skill-feedback-bridges-v1 P1 — {@code bong:crack_reading} payload handler。
 *
 * 解析 server 发来的裂读 JSON，写入 {@link CrackReadingHudStateStore}，
 * 供 {@link CrackReadingOverlay} 渲染。
 *
 * payload 格式（对应 Rust {@code CrackReadingPayload}）：
 * <pre>
 * {
 *   "target_entity_id": long,
 *   "entries": [{"meridian": string, "bracket": string, "has_circuit": bool, "is_dead_armor": bool}],
 *   "is_deep": bool,
 *   "display_ticks": long
 * }
 * </pre>
 */
public final class CrackReadingHandler {

    private CrackReadingHandler() {}

    /**
     * 处理来自 {@code bong:crack_reading} channel 的 JSON payload。
     *
     * @param jsonPayload UTF-8 解码后的 JSON 字符串
     */
    public static void handle(String jsonPayload) {
        try {
            JsonObject root = JsonParser.parseString(jsonPayload).getAsJsonObject();

            long targetEntityId = root.get("target_entity_id").getAsLong();
            boolean isDeep = root.has("is_deep") && root.get("is_deep").getAsBoolean();
            long displayTicks = root.has("display_ticks") ? root.get("display_ticks").getAsLong() : 60L;

            List<CrackReadingHudStateStore.MeridianEntry> entries = new ArrayList<>();
            if (root.has("entries") && root.get("entries").isJsonArray()) {
                JsonArray arr = root.getAsJsonArray("entries");
                for (JsonElement elem : arr) {
                    JsonObject e = elem.getAsJsonObject();
                    String meridian = e.has("meridian") ? e.get("meridian").getAsString() : "Unknown";
                    String bracket = e.has("bracket") ? e.get("bracket").getAsString() : "Intact";
                    boolean hasCircuit = e.has("has_circuit") && e.get("has_circuit").getAsBoolean();
                    boolean isDeadArmor = e.has("is_dead_armor") && e.get("is_dead_armor").getAsBoolean();
                    entries.add(new CrackReadingHudStateStore.MeridianEntry(meridian, bracket, hasCircuit, isDeadArmor));
                }
            }

            CrackReadingHudStateStore.accept(targetEntityId, entries, isDeep, displayTicks);
        } catch (Exception e) {
            // 解析失败不崩游戏，静默跳过
        }
    }
}
