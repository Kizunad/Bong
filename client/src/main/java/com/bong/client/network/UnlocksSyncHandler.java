package com.bong.client.network;

import com.bong.client.combat.UnlockedStyles;
import com.bong.client.combat.UnlockedStylesStore;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

/**
 * plan-HUD-v1 §1.3 / §11.4 unlocks_sync 客户端 handler。
 * server 在 `UnlockedStyles` Component 变化（join 首推 / 修炼解锁）时推送，
 * 本 handler 整体替换 {@link UnlockedStylesStore}，HUD planner 据此条件渲染
 * 流派指示器。
 */
public final class UnlocksSyncHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        boolean jiemai = readBool(payload, "jiemai");
        boolean tishi = readBool(payload, "tishi");
        boolean jueling = readBool(payload, "jueling");

        UnlockedStylesStore.replace(UnlockedStyles.of(jiemai, tishi, jueling));
        return ServerDataDispatch.handled(
            envelope.type(),
            "Applied unlocks_sync (jiemai=" + jiemai + " tishi=" + tishi
                + " jueling=" + jueling + ")"
        );
    }

    private static boolean readBool(JsonObject object, String fieldName) {
        JsonElement element = object.get(fieldName);
        if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) return false;
        JsonPrimitive primitive = element.getAsJsonPrimitive();
        return primitive.isBoolean() && primitive.getAsBoolean();
    }
}
