package com.bong.client.network;

/**
 * 非玩家实体动画触发 bridge：把 {@link VfxEventPayload.PlayEntityAnim} 转成"按 MC 协议
 * entity_id 找实体并触发其招式动画"。抽成接口与 {@link VfxParticleBridge} / {@link
 * VfxEventAnimationBridge} 一致，理由：
 * <ul>
 *   <li>单元测试：纯 JVM 环境下不能走 {@code MinecraftClient.world.getEntityById}，stub 一个
 *       记录型 bridge 就能验证路由分支</li>
 *   <li>生产实现（{@code FaunaActionBridge}）持有 MC 句柄，路由层与它解耦</li>
 * </ul>
 *
 * <p>返回值语义：
 * <ul>
 *   <li>{@code true}：目标实体在线且是支持招式动画的实体（{@code FaunaEntity}），已触发</li>
 *   <li>{@code false}：实体不在视距 / id 找不到 / 不是 {@code FaunaEntity} —— 由调用方做节流 warn</li>
 * </ul>
 */
@FunctionalInterface
public interface VfxEntityAnimationBridge {
    boolean playEntityAnim(int entityId, String anim, int durationTicks);

    /**
     * 空实现：协议已通但实体动画接线尚未就绪时使用。返回 false 让 router 记 bridgeMiss，
     * 避免悄悄吞掉事件。
     */
    static VfxEntityAnimationBridge noop() {
        return (entityId, anim, durationTicks) -> false;
    }
}
