package com.bong.client.network;

import com.bong.client.BongClient;
import com.google.gson.JsonObject;

/**
 * plan-weapon-v1 §6.3：{@code weapon_broken} payload 客户端 handler。
 *
 * <p>MVP 实现:log 一条 + 返回 handled。边缘闪烁 / HUD 通知可在未来扩展
 * (接 EventStream 或 ToastHud)。武器的 slot 清空由配套 {@code weapon_equipped
 * { weapon: null }} 完成,不在本 handler 里改 store。
 */
public final class WeaponBrokenHandler implements ServerDataHandler {
    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();
        long instanceId = payload.has("instance_id") ? payload.get("instance_id").getAsLong() : 0L;
        String templateId = payload.has("template_id")
            ? payload.get("template_id").getAsString()
            : "unknown";
        BongClient.LOGGER.info(
            "[bong][weapon] weapon broken: instance={} template={}",
            instanceId, templateId
        );
        return ServerDataDispatch.handled(
            envelope.type(),
            "Weapon broken: " + templateId + " (instance " + instanceId + ")"
        );
    }
}
