package com.bong.client.network;

import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.CombatHudStateStore;
import com.bong.client.combat.DerivedAttrFlags;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

/**
 * plan-HUD-v1 §11.4 {@code combat_hud_state} 客户端 handler。
 * 解析 server 推送的 hp/qi/stamina percent + DerivedAttrFlags，喂入
 * {@link CombatHudStateStore}（驱动左下角 mini body / 双竖条 / EdgeFeedback）。
 */
public final class CombatHudStateHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        Float hpPercent = readUnitFloat(payload, "hp_percent");
        Float qiPercent = readUnitFloat(payload, "qi_percent");
        Float staminaPercent = readUnitFloat(payload, "stamina_percent");
        Boolean combatActive = readRequiredBoolean(payload, "combat_active");
        JsonObject derivedObj = readObject(payload, "derived");
        if (hpPercent == null || qiPercent == null || staminaPercent == null
            || combatActive == null || derivedObj == null) {
            // 坏帧不能继续沿用旧的战斗判断，否则邀请策略会在权威输入失效后放行。
            CombatHudStateStore.clear();
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring combat_hud_state payload: required fields missing or invalid"
            );
        }

        Boolean flying = readRequiredBoolean(derivedObj, "flying");
        Boolean phasing = readRequiredBoolean(derivedObj, "phasing");
        Boolean tribulationLocked = readRequiredBoolean(derivedObj, "tribulation_locked");
        if (flying == null || phasing == null || tribulationLocked == null) {
            // derived flags 也是权威帧的一部分；缺失或类型错误时不能默认为 false。
            CombatHudStateStore.clear();
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring combat_hud_state payload: derived flags missing or invalid"
            );
        }

        CombatHudState next = CombatHudState.createAuthoritative(
            hpPercent,
            qiPercent,
            staminaPercent,
            DerivedAttrFlags.of(flying, phasing, tribulationLocked),
            combatActive
        );
        CombatHudStateStore.replaceAuthoritative(next);

        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied combat_hud_state (hp=" + hpPercent + " qi=" + qiPercent
                + " stam=" + staminaPercent + ")"
        );
    }

    private static Float readUnitFloat(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) return null;
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        if (!primitive.isNumber()) return null;
        double value = primitive.getAsDouble();
        if (!Double.isFinite(value) || value < 0.0 || value > 1.0) return null;
        return (float) value;
    }

    private static JsonObject readObject(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonObject()) return null;
        return element.getAsJsonObject();
    }

    private static Boolean readRequiredBoolean(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) return null;
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        return primitive.isBoolean() ? primitive.getAsBoolean() : null;
    }
}
