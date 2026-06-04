package com.bong.client.combat.handler;

import com.bong.client.hud.DuguV2HudStateStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

/**
 * plan-combat-skill-feedback-bridges-v1 P5：处理毒蛊 v2 HUD S2C payloads。
 *
 * <p>按 payload type（由 ServerDataRouter 路由的 key）做 per-dimension merge 更新
 * {@link DuguV2HudStateStore}，避免不同维度事件到来时互相覆盖归零（P4 教训）。
 *
 * <p>处理的 payload types：
 * <ul>
 *   <li>{@code dugu_v2_skill_cast}    — eclipse/penetrate/reverse 招式命中（瞬态，HUD flash）</li>
 *   <li>{@code dugu_v2_self_cure}     — 自蕴进度 + self_revealed sticky flag</li>
 *   <li>{@code dugu_v2_shroud_active} — 幻影遮蔽激活（含 expires_at_tick → shroudUntilMs）</li>
 *   <li>{@code permanent_qi_max_decay_applied} — 永久真元上限衰减通知（仅用于触发 HUD，不改 store qi 字段）</li>
 * </ul>
 *
 * <p>守恒红线：只读 payload 字段，不重算真元，不修改任何其他 store。
 */
public final class DuguV2ServerDataHandler implements ServerDataHandler {

    /**
     * 每个游戏 tick 约 50ms（20 TPS）；幻影遮蔽持续时间由 server 侧 expires_at_tick 推算。
     * 此常量用于 tick → ms 估算（仅 shroudUntilMs 计算用，误差在可接受范围内）。
     */
    private static final long MS_PER_TICK = 50L;

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        String type = envelope.type();

        switch (type) {
            case "dugu_v2_skill_cast" -> {
                // 瞬态事件：读 reveal_probability（eclipse/penetrate 招式携带），per-dimension merge 写 revealRisk。
                // 不改 shroud/selfCure/qi_decay 维度（P4 教训：只改本维度）。
                String kind = readString(payload, "kind");
                float revealProbability = (float) readDouble(payload, "reveal_probability", 0.0);
                DuguV2HudStateStore.State cur = DuguV2HudStateStore.snapshot();
                DuguV2HudStateStore.replace(new DuguV2HudStateStore.State(
                    cur.tainted(),
                    cur.taintIntensity(),
                    cur.taintHint(),
                    revealProbability,           // ← per-dimension merge: 仅更新 revealRisk
                    cur.selfCurePercent(),
                    cur.selfRevealed(),
                    cur.shroudActive(),
                    cur.shroudUntilMs()
                ));
                return ServerDataDispatch.handled(type, "dugu_v2 cast kind=" + kind + " revealRisk=" + revealProbability);
            }
            case "dugu_v2_self_cure" -> {
                float gainPercent = (float) readDouble(payload, "gain_percent", 0.0);
                boolean selfRevealed = readBool(payload, "self_revealed");
                DuguV2HudStateStore.State cur = DuguV2HudStateStore.snapshot();
                // self_revealed 是 sticky flag（once-set-stays-true）
                DuguV2HudStateStore.replace(new DuguV2HudStateStore.State(
                    cur.tainted(),
                    cur.taintIntensity(),
                    cur.taintHint(),
                    cur.revealRisk(),
                    gainPercent,
                    cur.selfRevealed() || selfRevealed,  // sticky: 一旦 true 不归零
                    cur.shroudActive(),
                    cur.shroudUntilMs()
                ));
                return ServerDataDispatch.handled(
                    type,
                    "self_cure gain=" + gainPercent + " revealed=" + (cur.selfRevealed() || selfRevealed)
                );
            }
            case "dugu_v2_shroud_active" -> {
                float strength = (float) readDouble(payload, "strength", 0.0);
                long expiresAtTick = readLong(payload, "expires_at_tick", 0L);
                long tick = readLong(payload, "tick", 0L);
                // 将 expires_at_tick 转换为绝对 ms（估算：currentTimeMillis + (expiresAtTick - tick) * MS_PER_TICK）
                long remainingTicks = Math.max(0L, expiresAtTick - tick);
                long shroudUntilMs = System.currentTimeMillis() + remainingTicks * MS_PER_TICK;
                DuguV2HudStateStore.State cur = DuguV2HudStateStore.snapshot();
                DuguV2HudStateStore.replace(new DuguV2HudStateStore.State(
                    cur.tainted(),
                    cur.taintIntensity(),
                    cur.taintHint(),
                    cur.revealRisk(),
                    cur.selfCurePercent(),
                    cur.selfRevealed(),
                    strength > 0f,
                    shroudUntilMs
                ));
                return ServerDataDispatch.handled(
                    type,
                    "shroud strength=" + strength + " expiresAtTick=" + expiresAtTick
                );
            }
            case "permanent_qi_max_decay_applied" -> {
                // 仅 HUD 通知用（loss/qi_max_after 只读，守恒红线：不在此处改 qi store）
                float loss = (float) readDouble(payload, "loss", 0.0);
                float qiMaxAfter = (float) readDouble(payload, "qi_max_after", 0.0);
                return ServerDataDispatch.handled(
                    type,
                    "permanent qi decay loss=" + loss + " qi_max_after=" + qiMaxAfter
                );
            }
            default -> {
                return ServerDataDispatch.noOp(type, "dugu_v2 handler: unknown type='" + type + "'");
            }
        }
    }

    // ─── JSON field readers (null-safe) ──────────────────────────

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
        double value = p.getAsDouble();
        return Double.isFinite(value) ? value : fallback;
    }

    private static long readLong(JsonObject obj, String field, long fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isNumber()) return fallback;
        long value = p.getAsLong();
        return value < 0 ? fallback : value;
    }

    private static boolean readBool(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return false;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isBoolean()) return false;
        return p.getAsBoolean();
    }
}
