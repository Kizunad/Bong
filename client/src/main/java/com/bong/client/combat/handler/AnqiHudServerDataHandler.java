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
 * plan-combat-skill-feedback-bridges-v1 P4 断链修复：
 * 处理 {@code anqi_hud} server-data payloads，将 server emit 的暗器 HUD 状态写入
 * {@link AnqiHudStateStore}，供 {@code AnqiHudPlanner.buildCommands()} 渲染。
 *
 * <p>payload.kind 路由：
 * <ul>
 *   <li>"echo" → {@link AnqiHudState#echo(int, long, long)}</li>
 *   <li>"aim"  → {@link AnqiHudState#aim(float, long, long)}</li>
 *   <li>"charge" → {@link AnqiHudState#charge(float, long, long)}</li>
 *   <li>"abrasion" → {@link AnqiHudState#abrasion(String, float, long, long)}</li>
 * </ul>
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
        long now = System.currentTimeMillis();

        AnqiHudState state = switch (kind) {
            case "echo" -> {
                int echoCount = (int) readLong(payload, "echo_count", 0L);
                yield AnqiHudState.echo(echoCount, now, DISPLAY_DURATION_MS);
            }
            case "aim" -> {
                float aimProgress = (float) readDouble(payload, "aim_progress", 0.0);
                yield AnqiHudState.aim(aimProgress, now, DISPLAY_DURATION_MS);
            }
            case "charge" -> {
                float chargeProgress = (float) readDouble(payload, "charge_progress", 0.0);
                yield AnqiHudState.charge(chargeProgress, now, DISPLAY_DURATION_MS);
            }
            case "abrasion" -> {
                String container = readString(payload, "abrasion_container");
                float qiPayload = (float) readDouble(payload, "abrasion_qi_payload", 0.0);
                yield AnqiHudState.abrasion(container, qiPayload, now, DISPLAY_DURATION_MS);
            }
            default -> {
                // 未知 kind 静默忽略，不修改 store
                yield null;
            }
        };

        if (state == null) {
            return ServerDataDispatch.noOp(
                    envelope.type(),
                    "anqi_hud: unknown kind='" + kind + "', ignoring safely"
            );
        }

        AnqiHudStateStore.replace(state);
        return ServerDataDispatch.handled(
                envelope.type(),
                "Applied anqi_hud kind=" + kind + " state=" + state
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
