package com.bong.client.network;

import com.bong.client.combat.EquippedWeapon;
import com.bong.client.combat.WeaponEquippedStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

/**
 * plan-weapon-v1 §8.2：{@code weapon_equipped} payload 客户端 handler。
 *
 * <p>{@code weapon == null/absent} 表示该 slot 被清空(卸下 / 武器 broken 后自动移除)。
 * 其余情况写入 {@link WeaponEquippedStore}。
 */
public final class WeaponEquippedHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        String slot = readString(payload, "slot", "main_hand");

        JsonElement weaponElem = payload.get("weapon");
        if (weaponElem == null || weaponElem.isJsonNull() || !weaponElem.isJsonObject()) {
            WeaponEquippedStore.putOrClear(slot, null);
            return ServerDataDispatch.handled(envelope.type(), "Cleared slot " + slot);
        }

        JsonObject w = weaponElem.getAsJsonObject();
        long instanceId = w.get("instance_id").getAsLong();
        String templateId = w.get("template_id").getAsString();
        String weaponKind = w.get("weapon_kind").getAsString();
        float durCurrent = w.get("durability_current").getAsFloat();
        float durMax = w.get("durability_max").getAsFloat();
        int qualityTier = w.get("quality_tier").getAsInt();

        WeaponEquippedStore.putOrClear(slot, new EquippedWeapon(
            slot, instanceId, templateId, weaponKind,
            durCurrent, durMax, qualityTier
        ));
        return ServerDataDispatch.handled(
            envelope.type(),
            "Equipped " + templateId + " to " + slot
                + " (dur=" + durCurrent + "/" + durMax
                + ", tier=" + qualityTier + ")"
        );
    }

    private static String readString(JsonObject obj, String field, String fallback) {
        JsonElement e = obj.get(field);
        if (e == null || !e.isJsonPrimitive()) return fallback;
        return e.getAsString();
    }
}
