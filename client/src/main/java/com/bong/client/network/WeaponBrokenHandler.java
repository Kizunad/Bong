package com.bong.client.network;

import com.bong.client.BongClient;
import com.bong.client.state.VisualEffectState;
import com.google.gson.JsonObject;

/**
 * plan-weapon-v1 §6.3：{@code weapon_broken} payload 客户端 handler。
 *
 * <p>MVP 实现:log 一条 + 返回 handled。边缘闪烁 / HUD 通知可在未来扩展
 * (接 EventStream 或 ToastHud)。武器的 slot 清空由配套 {@code weapon_equipped
 * { weapon: null }} 完成,不在本 handler 里改 store。
 */
public final class WeaponBrokenHandler implements ServerDataHandler {
    static final int BROKEN_TOAST_COLOR = 0xFFC04040;
    static final long BROKEN_TOAST_DURATION_MS = 2800L;
    static final long BROKEN_FLASH_DURATION_MS = 260L;
    static final double BROKEN_FLASH_INTENSITY = 1.0;

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
        return ServerDataDispatch.handledWithEventAlert(
            envelope.type(),
            new ServerDataDispatch.ToastSpec(
                "武器损坏：" + templateId,
                BROKEN_TOAST_COLOR,
                BROKEN_TOAST_DURATION_MS
            ),
            VisualEffectState.create(
                "weapon_break_flash",
                BROKEN_FLASH_INTENSITY,
                BROKEN_FLASH_DURATION_MS,
                System.currentTimeMillis()
            ),
            "Weapon broken: " + templateId + " (instance " + instanceId + ")"
        );
    }
}
