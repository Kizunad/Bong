package com.bong.client.combat.handler;

import com.bong.client.hud.AnqiHudState;
import com.bong.client.hud.AnqiHudStateStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.Set;

/**
 * plan-combat-skill-feedback-bridges-v1 P4 断链修复（@hive blocker+major 修复）：
 * 处理 {@code anqi_hud} server-data payloads，将 server emit 的暗器 HUD 状态写入
 * {@link AnqiHudStateStore}，供 {@code AnqiHudPlanner.buildCommands()} 渲染。
 *
 * <p>payload.kind 路由（server 实际发送的各路）：
 * <ul>
 *   <li>"echo"      → {@link AnqiHudStateStore#updateEcho}（DecoyDeployEvent.echo_count）</li>
 *   <li>"aim"       → {@link AnqiHudStateStore#updateAim}（协议预留的瞄准进度）</li>
 *   <li>"charge"    → {@link AnqiHudStateStore#updateCharge}（QiInjectionEvent.overload_ratio /
 *       ArmorPierceEvent.ignored_defense_ratio 作蓄力度量）</li>
 *   <li>"abrasion"  → {@link AnqiHudStateStore#updateAbrasion}（CarrierAbrasionEvent.after_qi）</li>
 *   <li>"multishot" → {@link AnqiHudStateStore#updateMultiShot}（MultiShotEvent.projectile_count，
 *       server 复用 echo_count 字段承载弹数）</li>
 * </ul>
 *
 * <p>并发修复（blocker）：每个 kind 只更新对应维度，不清零其他维度。三路事件并存时
 * echo/charge/abrasion 各维度独立 expiry，snapshot() 合并所有未过期维度供 Planner 渲染。
 *
 * <p>tick 守序修复（major）：payload 带 tick 字段，handler 传给 Store；Store 按维度记录
 * lastTick，仅 tick >= lastTick 才更新，乱序旧包静默丢弃。同帧不同维度允许相同 tick。
 *
 * <p>anqi_v2 当前没有生产 aim 事件源，但 {@code aim} 已是 Rust/TypeBox/protobuf 的正式 wire
 * 变体。handler 必须消费该变体，避免未来发送端接入后出现 schema 通过但 HUD 静默丢包。
 *
 * <p>守恒红线：只读 payload 字段，不重算真元，不修改任何其他 store。
 */
public final class AnqiHudServerDataHandler implements ServerDataHandler {

    /** 默认显示时长（毫秒），足够玩家观察 HUD 反馈。 */
    private static final long DISPLAY_DURATION_MS = 2_000L;
    private static final long MAX_SAFE_TICK = 9_007_199_254_740_991L;
    private static final long MAX_ECHO_COUNT = Integer.MAX_VALUE;
    // 略低于 Float.MAX_VALUE 的 double 精确值：收窄仍为 Float.MAX_VALUE，更大 wire 值在转换前拒绝。
    private static final double MAX_ABRASION_QI_PAYLOAD = 3.4028234e38;
    private static final Set<String> REQUIRED_FIELDS = Set.of(
            "v",
            "type",
            "kind",
            "echo_count",
            "aim_progress",
            "charge_progress",
            "abrasion_container",
            "abrasion_qi_payload",
            "tick");

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        if (!payload.keySet().equals(REQUIRED_FIELDS)) {
            return invalid(envelope, "payload fields must exactly match the anqi_hud v1 schema");
        }
        String kind = readString(payload, "kind");
        Long echoCount = readBoundedInteger(payload, "echo_count", MAX_ECHO_COUNT);
        Double aimProgress = readBoundedDouble(payload, "aim_progress", 0.0, 1.0);
        Double chargeProgress = readBoundedDouble(payload, "charge_progress", 0.0, 1.0);
        String container = readString(payload, "abrasion_container");
        Double qiPayload = readBoundedDouble(
                payload, "abrasion_qi_payload", 0.0, MAX_ABRASION_QI_PAYLOAD);
        Long tickValue = readBoundedInteger(payload, "tick", MAX_SAFE_TICK);
        if (kind == null
                || echoCount == null
                || aimProgress == null
                || chargeProgress == null
                || container == null
                || !isCanonicalContainer(container)
                || qiPayload == null
                || tickValue == null) {
            return invalid(envelope, "one or more fields violate the anqi_hud v1 schema");
        }

        long now  = System.currentTimeMillis();
        long tick = tickValue;

        switch (kind) {
            case "echo" ->
                AnqiHudStateStore.updateEcho(echoCount.intValue(), now, DISPLAY_DURATION_MS, tick);
            case "aim" ->
                AnqiHudStateStore.updateAim(aimProgress.floatValue(), now, DISPLAY_DURATION_MS, tick);
            case "charge" ->
                AnqiHudStateStore.updateCharge(
                        chargeProgress.floatValue(), now, DISPLAY_DURATION_MS, tick);
            case "abrasion" ->
                AnqiHudStateStore.updateAbrasion(
                        container, qiPayload.floatValue(), now, DISPLAY_DURATION_MS, tick);
            case "multishot" ->
                // 多发齐射：server 用 echo_count 字段承载 projectile_count（复用字段，无新 proto 字段）。
                AnqiHudStateStore.updateMultiShot(
                        echoCount.intValue(), now, DISPLAY_DURATION_MS, tick);
            default -> {
                // 未知 kind 静默忽略，不修改 store。
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
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        JsonPrimitive p = el.getAsJsonPrimitive();
        return p.isString() ? p.getAsString() : null;
    }

    private static Double readBoundedDouble(
            JsonObject obj,
            String field,
            double minimum,
            double maximum) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isNumber()) return null;
        try {
            double value = p.getAsDouble();
            return Double.isFinite(value) && value >= minimum && value <= maximum
                    ? value
                    : null;
        } catch (NumberFormatException error) {
            return null;
        }
    }

    private static Long readBoundedInteger(JsonObject obj, String field, long maximum) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (!p.isNumber()) return null;
        try {
            double numeric = p.getAsDouble();
            if (!Double.isFinite(numeric)
                    || numeric < 0.0
                    || numeric > maximum
                    || numeric != Math.rint(numeric)) {
                return null;
            }
            long value = p.getAsLong();
            return value >= 0 && value <= maximum && (double) value == numeric ? value : null;
        } catch (NumberFormatException error) {
            return null;
        }
    }

    private static boolean isCanonicalContainer(String value) {
        return value.isEmpty()
                || value.equals("hand_slot")
                || value.equals("quiver")
                || value.equals("pocket_pouch")
                || value.equals("fenglinghe");
    }

    private static ServerDataDispatch invalid(ServerDataEnvelope envelope, String reason) {
        return ServerDataDispatch.noOp(envelope.type(), "anqi_hud: invalid payload: " + reason);
    }
}
