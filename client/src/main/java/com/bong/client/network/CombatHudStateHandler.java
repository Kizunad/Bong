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
        JsonObject derivedObj = readObject(payload, "derived");
        if (hpPercent == null || qiPercent == null || staminaPercent == null || derivedObj == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "Ignoring combat_hud_state payload: required fields missing or invalid"
            );
        }

        boolean flying = readBool(derivedObj, "flying");
        boolean phasing = readBool(derivedObj, "phasing");
        boolean tribulationLocked = readBool(derivedObj, "tribulation_locked");

        CombatHudState next = CombatHudState.create(
            hpPercent,
            qiPercent,
            staminaPercent,
            DerivedAttrFlags.of(flying, phasing, tribulationLocked)
        );
        CombatHudStateStore.replace(next);

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

    private static boolean readBool(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) return false;
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        return primitive.isBoolean() && primitive.getAsBoolean();
    }
}
