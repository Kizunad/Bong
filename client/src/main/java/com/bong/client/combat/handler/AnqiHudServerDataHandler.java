package com.bong.client.combat.handler;

import com.bong.client.hud.AnqiHudState;
import com.bong.client.hud.AnqiHudStateStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

/**
 * plan-combat-skill-feedback-bridges-v1 P4 断链修复（@hive blocker+major 修复）：
 * 处理 {@code anqi_hud} server-data payloads，将 server emit 的暗器 HUD 状态写入
 * {@link AnqiHudStateStore}，供 {@code AnqiHudPlanner.buildCommands()} 渲染。
 *
 * <p>payload.kind 路由（当前 server 实际发送的三路）：
 * <ul>
 *   <li>"echo"     → {@link AnqiHudStateStore#updateEcho}（DecoyDeployEvent.echo_count）</li>
 *   <li>"charge"   → {@link AnqiHudStateStore#updateCharge}（QiInjectionEvent.overload_ratio 作蓄力度量）</li>
 *   <li>"abrasion" → {@link AnqiHudStateStore#updateAbrasion}（CarrierAbrasionEvent.after_qi）</li>
 * </ul>
 *
 * <p>并发修复（blocker）：每个 kind 只更新对应维度，不清零其他维度。三路事件并存时
 * echo/charge/abrasion 各维度独立 expiry，snapshot() 合并所有未过期维度供 Planner 渲染。
 *
 * <p>tick 守序修复（major）：payload 带 tick 字段，handler 传给 Store；Store 按维度记录
 * lastTick，仅 tick >= lastTick 才更新，乱序旧包静默丢弃。同帧不同维度允许相同 tick。
 *
 * <p>"aim" 路由暂不实现：anqi_v2 当前无 aim 前摇/进度事件源，aim HUD 延后至未来
 * 引入 aim-phase 事件的 plan。{@link AnqiHudState#aim} factory 及 {@code AnqiHudPlanner.appendAim}
 * 留作预留接口，未来 plan 接入时无需改动 handler 以外的代码。
 *
 * <p>守恒红线：只读 payload 字段，不重算真元，不修改任何其他 store。
 */
public final class AnqiHudServerDataHandler implements ServerDataHandler {

    /** 默认显示时长（毫秒），足够玩家观察 HUD 反馈。 */
    private static final long DISPLAY_DURATION_MS = 2_000L;

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        String kind = readString(payload, "kind");
        long now  = System.currentTimeMillis();
        long tick = readLong(payload, "tick", 0L);

        switch (kind) {
            case "echo" -> {
                int echoCount = (int) readLong(payload, "echo_count", 0L);
                AnqiHudStateStore.updateEcho(echoCount, now, DISPLAY_DURATION_MS, tick);
            }
            case "charge" -> {
                float chargeProgress = (float) readDouble(payload, "charge_progress", 0.0);
                AnqiHudStateStore.updateCharge(chargeProgress, now, DISPLAY_DURATION_MS, tick);
            }
            case "abrasion" -> {
                String container = readString(payload, "abrasion_container");
                float qiPayload = (float) readDouble(payload, "abrasion_qi_payload", 0.0);
                AnqiHudStateStore.updateAbrasion(container, qiPayload, now, DISPLAY_DURATION_MS, tick);
            }
            default -> {
                // 未知 kind（包括 "aim"——server 当前不发，预留给未来 aim-phase plan）静默忽略，不修改 store
                return ServerDataDispatch.noOp(
                        envelope.type(),
                        "anqi_hud: unknown kind='" + kind + "', ignoring safely"
                );
            }
        }

        return ServerDataDispatch.handled(
                envelope.type(),
                "Applied anqi_hud kind=" + kind + " tick=" + tick
        );
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
}
