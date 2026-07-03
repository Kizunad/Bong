package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import com.bong.client.scroll.ScrollReadAudio;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

/**
 * {@code bong:scroll_open_glow} —— 卷轴展开时的淡金色微光（plan-scroll-reading-v1 P2）。
 *
 * <p>复用既有 {@link BongParticles#enlightenmentDustSprites}（不新增贴图，同
 * {@link BedRestAuraPlayer} 的复用惯例）。规格：burst 12 粒短寿命迸出（模拟"展卷"一瞬），
 * 叠加 continuous 10 粒长寿命缓慢上飘（模拟 1 粒/2tick × 20tick 的持续微光感——单次 server
 * emit 无逐 tick 调度接口，用较长 maxAge + 较慢上升速度在视觉上还原"持续"而非"一瞬闪过"）。
 * 默认色 {@code #E8D9A0} 淡金，自玩家胸前 0.4m 半径球面向外漂。
 *
 * <p><b>音效</b>：同一事件顺带触发"开卷"音效（{@link ScrollReadAudio#playOpen()}），仿
 * {@code PackOperationVfxPlayer} 的"单 VfxEvent 驱动粒子+音效组合反馈"模式——server 只发一次
 * {@code spawn_particle}，client 侧粒子与音效均由本类统一触发，不新增网络契约。
 */
public final class ScrollOpenGlowPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "scroll_open_glow");

    private static final int FALLBACK_RGB = 0xE8D9A0;
    private static final int BURST_COUNT = 12;
    private static final int CONTINUOUS_COUNT = 10;
    private static final int BURST_MAX_AGE = 10;
    private static final int CONTINUOUS_MAX_AGE = 40;
    private static final double RADIUS = 0.4;
    private static final float MIN_ALPHA = 0.5f;
    private static final float MAX_ALPHA = 0.95f;
    private static final int MIN_COUNT = 1;
    private static final int MAX_COUNT = 32;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) return;

        // 胸前 0.4m 高度偏移（origin 是脚下坐标，胸前大致躯干中段）。
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 1.2;
        double oz = payload.origin()[2];

        float[] rgb = GameplayVfxUtil.rgb(payload, FALLBACK_RGB);
        float alpha = (float) GameplayVfxUtil.clamp(payload.strength().orElse(0.85), MIN_ALPHA, MAX_ALPHA);
        int burstCount = GameplayVfxUtil.clamp(payload.count().orElse(BURST_COUNT), MIN_COUNT, MAX_COUNT);

        // burst：短寿命迸出，速度稍快，模拟"展卷一瞬"的迸溅感。
        for (int i = 0; i < burstCount; i++) {
            double[] offset = sphereOffset(world);
            double vx = offset[0] * 0.03;
            double vy = 0.02 + world.random.nextDouble() * 0.02;
            double vz = offset[2] * 0.03;
            EnlightenmentAuraPlayer.spawnSprite(client, world, BongParticles.enlightenmentDustSprites,
                ox + offset[0] * RADIUS, oy + offset[1] * RADIUS, oz + offset[2] * RADIUS,
                vx, vy, vz, rgb[0], rgb[1], rgb[2], alpha, BURST_MAX_AGE, 0.05f);
        }

        // continuous：长寿命缓慢上飘，视觉上还原持续微光（非一瞬即逝）。
        for (int i = 0; i < CONTINUOUS_COUNT; i++) {
            double[] offset = sphereOffset(world);
            double vx = offset[0] * 0.01;
            double vy = 0.008 + world.random.nextDouble() * 0.012;
            double vz = offset[2] * 0.01;
            EnlightenmentAuraPlayer.spawnSprite(client, world, BongParticles.enlightenmentDustSprites,
                ox + offset[0] * RADIUS, oy + offset[1] * RADIUS, oz + offset[2] * RADIUS,
                vx, vy, vz, rgb[0], rgb[1], rgb[2], alpha * 0.8f, CONTINUOUS_MAX_AGE, 0.04f);
        }

        ScrollReadAudio.playOpen();
    }

    /** 球面上的单位向量偏移（用于"自胸前 0.4m 半径球面向外漂"的分布形状）。 */
    private static double[] sphereOffset(ClientWorld world) {
        double theta = world.random.nextDouble() * Math.PI * 2;
        double phi = world.random.nextDouble() * Math.PI;
        double x = Math.sin(phi) * Math.cos(theta);
        double y = Math.cos(phi);
        double z = Math.sin(phi) * Math.sin(theta);
        return new double[] {x, y, z};
    }
}
