package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.particle.ParticleTypes;
import net.minecraft.util.Identifier;

/**
 * {@code bong:guangbo_ticao_practice} —— 广播体操（body.guangbo_ticao）练习完成的正反馈粒子。
 *
 * <p>视觉为 vanilla {@link ParticleTypes#HAPPY_VILLAGER}（绿色心叶星点），围绕练习者头部上方
 * 小范围散开。复用 vanilla 粒子贴图，无净新资产。
 *
 * <p>架构对齐：与 woliu / anqi / baomai / lingtian 等所有 gameplay 粒子一致，走 {@code bong:}
 * 命名空间事件 + {@code bong:vfx_event} JSON 通道（{@link VfxRegistry} 查表），从而统一享受
 * 服务端的半径过滤 / 合批 / 优先级限流。服务端 {@code emit_guangbo_ticao_visual_triggers} 引用
 * 此 event id；{@link VfxBootstrap#registerDefaults()} 完成注册。
 */
public final class GuangboTicaoPracticePlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "guangbo_ticao_practice");

    private static final int DEFAULT_COUNT = 6;
    private static final int MIN_COUNT = 1;
    private static final int MAX_COUNT = 16;
    private static final double SPREAD_RADIUS = 0.45;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }
        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];
        int count = GameplayVfxUtil.count(payload, DEFAULT_COUNT, MIN_COUNT, MAX_COUNT);

        for (int i = 0; i < count; i++) {
            double dx = (world.random.nextDouble() - 0.5) * 2.0 * SPREAD_RADIUS;
            double dz = (world.random.nextDouble() - 0.5) * 2.0 * SPREAD_RADIUS;
            double dy = world.random.nextDouble() * 0.5;
            // 轻微向上漂移，呈现“鼓励/进步”的上扬星点。
            world.addParticle(
                ParticleTypes.HAPPY_VILLAGER,
                ox + dx,
                oy + dy,
                oz + dz,
                0.0,
                0.04 + world.random.nextDouble() * 0.03,
                0.0
            );
        }
    }
}
