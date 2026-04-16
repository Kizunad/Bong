package com.bong.client.network;

import com.bong.client.combat.DefenseStanceState;
import com.bong.client.combat.DefenseStanceStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

/**
 * plan-HUD-v1 §3.4 / §11.4 defense_sync 客户端 handler。
 * server 在 `DefenseStance` Component 变化（切换姿态 / 伪皮层数 / 涡流冷却）时推送，
 * 本 handler 整体替换 {@link DefenseStanceStore}，对应 HUD planner 据此渲染流派指示器。
 */
public final class DefenseSyncHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        String stanceStr = readString(payload, "stance");
        Long fakeSkin = readLong(payload, "fake_skin_layers");
        Boolean vortexActive = readBool(payload, "vortex_active");
        Long vortexReadyAt = readLong(payload, "vortex_ready_at_ms");

        if (stanceStr == null || fakeSkin == null
            || vortexActive == null || vortexReadyAt == null
            || fakeSkin < 0 || vortexReadyAt < 0) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring defense_sync payload: required fields missing or invalid");
        }

        DefenseStanceState.Stance stance = parseStance(stanceStr);
        if (stance == null) {
            return ServerDataDispatch.noOp(envelope.type(),
                "Ignoring defense_sync payload: unknown stance '" + stanceStr + "'");
        }

        DefenseStanceStore.replace(DefenseStanceState.of(
            stance,
            fakeSkin.intValue(),
            vortexActive,
            vortexReadyAt
        ));
        return ServerDataDispatch.handled(envelope.type(),
            "Applied defense_sync (stance=" + stanceStr + " layers=" + fakeSkin
                + " vortex=" + vortexActive + ")");
    }

    private static DefenseStanceState.Stance parseStance(String wire) {
        return switch (wire) {
            case "none" -> DefenseStanceState.Stance.NONE;
            case "jiemai" -> DefenseStanceState.Stance.JIEMAI;
            case "tishi" -> DefenseStanceState.Stance.TISHI;
            case "jueling" -> DefenseStanceState.Stance.JUELING;
            default -> null;
        };
    }

    private static String readString(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        JsonPrimitive p = el.getAsJsonPrimitive();
        return p.isString() ? p.getAsString() : null;
    }

    private static Long readLong(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        JsonPrimitive p = el.getAsJsonPrimitive();
        return p.isNumber() ? p.getAsLong() : null;
    }

    private static Boolean readBool(JsonObject obj, String field) {
        JsonElement el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) return null;
        JsonPrimitive p = el.getAsJsonPrimitive();
        return p.isBoolean() ? p.getAsBoolean() : null;
    }
}
