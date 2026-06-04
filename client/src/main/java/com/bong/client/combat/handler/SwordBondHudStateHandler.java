package com.bong.client.combat.handler;

import com.bong.client.hud.SwordBondHudState;
import com.bong.client.hud.SwordBondHudStateStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataHandler;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

/**
 * plan-combat-skill-feedback-bridges-v1 P6 断链修复：
 * 处理 {@code sword_bond_hud_state} server-data payloads，将 server emit 的人剑共生 HUD 状态写入
 * {@link SwordBondHudStateStore}，供 {@code SwordPathHudPlanner.buildCommands()} 渲染。
 *
 * <p>走 ServerDataRouter handler-map 模式（仿 DuguPoisonStateHandler / AnqiHudServerDataHandler），
 * 非 applyDispatch getter 路径（ServerDataDispatch.swordBondHudState() 不存在）。
 *
 * <p>守恒红线：只读 payload 字段，不重算真元，不修改任何其他 store。
 * stored_qi_ratio 由 server 计算（stored_qi / cap），client 只负责写入渲染 store。
 */
public final class SwordBondHudStateHandler implements ServerDataHandler {

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();

        boolean active = readBoolean(payload, "active", false);

        if (!active) {
            // active=false：server 通知无 SwordBondComponent，清空 store
            SwordBondHudStateStore.replace(SwordBondHudState.INACTIVE);
            return ServerDataDispatch.handled(
                    envelope.type(),
                    "sword_bond_hud_state: active=false, store reset to INACTIVE"
            );
        }

        int gradeIndex = (int) readLong(payload, "grade_index", 0L);
        String gradeName = readString(payload, "grade_name");
        float storedQiRatio = (float) readDouble(payload, "stored_qi_ratio", 0.0);
        float bondStrength = (float) readDouble(payload, "bond_strength", 0.0);
        boolean heavenGateReady = readBoolean(payload, "heaven_gate_ready", false);

        SwordBondHudState state = new SwordBondHudState(
                true,
                gradeIndex,
                gradeName,
                storedQiRatio,
                bondStrength,
                heavenGateReady
        );
        SwordBondHudStateStore.replace(state);

        return ServerDataDispatch.handled(
                envelope.type(),
                "Applied sword_bond_hud_state grade=" + gradeIndex + "(" + gradeName + ")"
                        + " storedQiRatio=" + storedQiRatio
                        + " heavenGate=" + heavenGateReady
        );
    }

    // ─── JSON field readers (null-safe) ──────────────────────────

    private static String readString(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return "";
        JsonPrimitive p = el.getAsJsonPrimitive();
        return p.isString() ? p.getAsString() : "";
    }

    private static boolean readBoolean(JsonObject obj, String field, boolean fallback) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return fallback;
        JsonPrimitive p = el.getAsJsonPrimitive();
        if (p.isBoolean()) return p.getAsBoolean();
        if (p.isNumber()) return p.getAsDouble() != 0d;
        return fallback;
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
