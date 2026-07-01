package com.bong.client.cultivation;

/**
 * F7：{@link BreakthroughCinematicHandler} 处理 {@code breakthrough_cinematic} payload 时
 * 缓存的逐帧渲染状态，供 {@link BreakthroughBillboardWorldRenderer} 每帧读取。
 *
 * <p>{@code expiresAtMillis} 用 handler 里已经算好的
 * {@code BreakthroughSpectacleRenderer.SpectaclePlan#visualDurationMillis()} 推算——
 * 一旦当前阶段的演出时长过去，billboard 应自动消失，不需要等下一条 payload 覆盖。
 *
 * @param payload         最近一次收到的突破演出 payload（不可为 null，见 {@link #BreakthroughRenderState}）
 * @param expiresAtMillis 该 payload 对应演出阶段的到期时间（{@code System.currentTimeMillis()} 基准）
 */
public record BreakthroughRenderState(BreakthroughCinematicPayload payload, long expiresAtMillis) {
    public BreakthroughRenderState {
        if (payload == null) {
            throw new IllegalArgumentException("payload must not be null");
        }
    }

    public boolean isExpired(long nowMillis) {
        return nowMillis >= expiresAtMillis;
    }
}
