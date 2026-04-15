package com.bong.client.network;

/**
 * 粒子触发 bridge：把 {@link VfxEventPayload.SpawnParticle} 转成客户端粒子引擎调用。
 *
 * <p>plan-particle-system-v1 §2.7 规定客户端侧有一张 {@code VfxRegistry}：
 * {@code event_id → VfxPlayer}。本接口是 router 与那张表之间的边界，抽成接口是为了：
 * <ul>
 *   <li>单元测试：纯 JVM 环境下不能走 {@code MinecraftClient.particleManager}，stub 一个记录型
 *       bridge 就能验证路由分支</li>
 *   <li>Phase 1 渲染基类（Line / Ribbon / GroundDecal）实现完成前，router 能先用 {@link #noop()}
 *       占位，不阻塞协议层落地</li>
 * </ul>
 *
 * <p>返回值语义：
 * <ul>
 *   <li>{@code true}：事件被注册表接住（注意：不代表帧内就渲染了，{@code VfxPlayer} 内部可能
 *       有自己的节流）</li>
 *   <li>{@code false}：{@code event_id} 未注册 / 客户端尚未就绪（无 world），由调用方做节流 warn</li>
 * </ul>
 */
@FunctionalInterface
public interface VfxParticleBridge {
    boolean spawnParticle(VfxEventPayload.SpawnParticle payload);

    /**
     * 空实现：协议已通但渲染基类尚未接入时使用。
     * 返回 false 以便 router 记 bridgeMiss，让上层在日志里看到 "未注册"，
     * 避免悄悄吞掉事件。
     */
    static VfxParticleBridge noop() {
        return payload -> false;
    }
}
