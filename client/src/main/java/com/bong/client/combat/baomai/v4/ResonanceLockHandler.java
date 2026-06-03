package com.bong.client.combat.baomai.v4;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

/**
 * plan-combat-skill-feedback-bridges-v1 P1 — 共振锁定 payload handler。
 *
 * 处理两个 channel（同一 handler，按 channel 区分逻辑）：
 * <ul>
 *   <li>{@code bong:resonance_lock} — 锁定开始，写入 started_at / ends_at</li>
 *   <li>{@code bong:resonance_lock_end} — 锁定结束，清空状态</li>
 * </ul>
 *
 * resonance_lock payload 格式：
 * <pre>
 * {"v":1,"fighter_a_id":string,"fighter_b_id":string,"started_at":long,"ends_at":long}
 * </pre>
 *
 * resonance_lock_end payload 格式：
 * <pre>
 * {"v":1,"fighter_a_id":string,"fighter_b_id":string,"reason":{...},"tick":long}
 * </pre>
 *
 * 差异化 VFX：锁定开始时通过 {@link ResonanceLockVfxTrigger} 触发锁定闪光粒子，
 * 与 crack_reading overlay 和 iron_cocoon event_flow 完全不同的视觉路径。
 */
public final class ResonanceLockHandler {

    /** 本玩家的 entity wire id（在 connect 时写入，默认 null = 未初始化）。 */
    private static volatile String localPlayerId = null;

    private ResonanceLockHandler() {}

    /** 设置本玩家的 entity wire id（BongNetworkHandler register 时调用一次）。 */
    public static void setLocalPlayerId(String playerId) {
        localPlayerId = playerId;
    }

    /**
     * 处理 {@code bong:resonance_lock} payload。
     *
     * @param jsonPayload UTF-8 解码后的 JSON 字符串
     */
    public static void handleLock(String jsonPayload) {
        try {
            JsonObject root = JsonParser.parseString(jsonPayload).getAsJsonObject();
            String fighterId = extractLocalFighter(root);
            String partnerId = extractPartner(root, fighterId);
            long startedAt = root.get("started_at").getAsLong();
            long endsAt = root.get("ends_at").getAsLong();

            ResonanceLockHudStateStore.onLockStarted(partnerId, startedAt, endsAt);

            // 触发锁定 VFX（仿共振冲击波，颜色：深紫 #7B2FBE，粒子数 24，生命 40 tick）
            ResonanceLockVfxTrigger.onLockStarted();
        } catch (Exception e) {
            // 解析失败静默跳过
        }
    }

    /**
     * 处理 {@code bong:resonance_lock_end} payload。
     *
     * @param jsonPayload UTF-8 解码后的 JSON 字符串
     */
    public static void handleLockEnd(String jsonPayload) {
        try {
            // 只需清空 HUD 状态，payload 内容此处不需要细解析
            ResonanceLockHudStateStore.onLockEnded();
            ResonanceLockVfxTrigger.onLockEnded();
        } catch (Exception e) {
            // 静默跳过
        }
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    private static String extractLocalFighter(JsonObject root) {
        String a = root.has("fighter_a_id") ? root.get("fighter_a_id").getAsString() : "";
        String b = root.has("fighter_b_id") ? root.get("fighter_b_id").getAsString() : "";
        String local = localPlayerId;
        if (local != null && local.equals(a)) return a;
        if (local != null && local.equals(b)) return b;
        return a; // fallback
    }

    private static String extractPartner(JsonObject root, String localId) {
        String a = root.has("fighter_a_id") ? root.get("fighter_a_id").getAsString() : "";
        String b = root.has("fighter_b_id") ? root.get("fighter_b_id").getAsString() : "";
        if (localId.equals(a)) return b;
        if (localId.equals(b)) return a;
        return b; // fallback
    }
}
