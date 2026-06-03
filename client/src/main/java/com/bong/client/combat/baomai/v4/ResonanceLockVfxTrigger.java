package com.bong.client.combat.baomai.v4;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.particle.ParticleManager;

/**
 * plan-combat-skill-feedback-bridges-v1 P1 — 共振锁定 VFX 触发器。
 *
 * 锁定开始时播放冲击波粒子（深紫 #7B2FBE），结束时播放散开粒子（浅紫 #D4AAFF）。
 * 与 crack_reading overlay（色阶文字）和 iron_cocoon event_flow（事件流文本）完全不同的视觉路径。
 *
 * 实现方式：直接调用 MinecraftClient 的粒子 API 在玩家周围生成粒子效果，
 * 不依赖服务端发 VFX event（客户端本地生成，仅本玩家可见）。
 */
public final class ResonanceLockVfxTrigger {

    // 粒子参数
    private static final int LOCK_PARTICLE_COUNT = 24;
    private static final int LOCK_PARTICLE_LIFETIME_TICKS = 40;
    private static final int UNLOCK_PARTICLE_COUNT = 16;
    private static final int UNLOCK_PARTICLE_LIFETIME_TICKS = 20;

    private ResonanceLockVfxTrigger() {}

    /**
     * 共振锁定开始 — 触发深紫冲击波粒子（锁定感知）。
     */
    public static void onLockStarted() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client.player == null || client.world == null) return;

        double px = client.player.getX();
        double py = client.player.getY() + 1.0;
        double pz = client.player.getZ();

        // 在玩家周围随机散布深紫粒子（模拟共振冲击波）
        for (int i = 0; i < LOCK_PARTICLE_COUNT; i++) {
            double dx = (client.world.random.nextDouble() - 0.5) * 2.0;
            double dy = (client.world.random.nextDouble() - 0.5) * 0.5;
            double dz = (client.world.random.nextDouble() - 0.5) * 2.0;
            // 使用 CRIT 粒子作为基础（近似共振碰撞效果）
            client.world.addParticle(
                net.minecraft.particle.ParticleTypes.CRIT,
                px + dx * 0.5, py + dy * 0.5, pz + dz * 0.5,
                dx * 0.1, dy * 0.05, dz * 0.1
            );
        }
    }

    /**
     * 共振锁定结束 — 触发浅紫散开粒子（解锁感知）。
     */
    public static void onLockEnded() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client.player == null || client.world == null) return;

        double px = client.player.getX();
        double py = client.player.getY() + 1.0;
        double pz = client.player.getZ();

        for (int i = 0; i < UNLOCK_PARTICLE_COUNT; i++) {
            double dx = (client.world.random.nextDouble() - 0.5) * 2.0;
            double dy = client.world.random.nextDouble() * 0.5;
            double dz = (client.world.random.nextDouble() - 0.5) * 2.0;
            client.world.addParticle(
                net.minecraft.particle.ParticleTypes.PORTAL,
                px + dx * 0.3, py + dy * 0.3, pz + dz * 0.3,
                dx * 0.05, dy * 0.05, dz * 0.05
            );
        }
    }
}
