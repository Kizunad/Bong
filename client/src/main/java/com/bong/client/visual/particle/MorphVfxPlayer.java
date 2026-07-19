package com.bong.client.visual.particle;

import com.bong.client.hud.MorphCastVignetteState;
import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

/**
 * plan-race-system-v1 PR-5b — 易形（{@code morph.yixing}）施法特效。
 *
 * <p>视觉上与其余变形/蜕变类技能（如暗紫色调的 {@code heiwushi_transform}）区分：
 * 淡青白基调，螺旋缠绕上升 + 白雾扩散，象征"本体轮廓消融、易形为他物"。
 *
 * <p>规格锁定（plan §P4 视听规格表）：
 * <ul>
 *   <li>{@link #SPIRAL_RIBBON_COUNT}=24 条 {@link BongRibbonParticle} 螺旋（自下而上），
 *       lifetime 取 server 下发的 {@code duration_ticks}（缺省回落
 *       {@link #SPIRAL_LIFETIME_TICKS}），随 morph_cast 动画 endTick 对齐，色 {@code #E8DFC8}，
 *       复用 {@link BongParticles#vortexSpiralSprites}（无专属贴图，最接近的既有涡旋
 *       sprite provider）。</li>
 *   <li>{@link #MIST_SPRITE_COUNT}=40 粒 {@link BongSpriteParticle} 白雾 radial burst，
 *       lifetime 为 {@link #MIST_LIFETIME_TICKS} 与 {@code duration_ticks} 的较小值
 *       （雾层始终不长于螺旋主视觉），复用 {@link BongParticles#cloudDustSprites}
 *       （仓库无字面 "mist" 贴图 key，{@code cloudDustSprites} 是既有 provider 里视觉最贴近
 *       "雾" 的一个）。</li>
 * </ul>
 */
public final class MorphVfxPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "morph_yixing");

    private static final int SPIRAL_RIBBON_COUNT = 24;
    private static final int SPIRAL_LIFETIME_TICKS = 30;
    private static final int MIST_SPRITE_COUNT = 40;
    private static final int MIST_LIFETIME_TICKS = 20;
    /** #E8DFC8（plan §P4 视听规格表锁定色）——与 heiwushi_transform 的暗紫区分。 */
    private static final int FALLBACK_RGB = 0xE8DFC8;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) return;

        double[] origin = payload.origin();
        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        int spiralLifetime = payload.durationTicks().orElse(SPIRAL_LIFETIME_TICKS);

        spawnSpiral(client, world, origin, r, g, b, spiralLifetime);
        spawnMist(client, world, origin, Math.min(MIST_LIFETIME_TICKS, spiralLifetime));
        // plan §P4 视听规格表——施法期白色 vignette，MorphHudPlanner 消费。
        MorphCastVignetteState.trigger(System.currentTimeMillis());
    }

    private static void spawnSpiral(
        MinecraftClient client,
        ClientWorld world,
        double[] origin,
        float r,
        float g,
        float b,
        int lifetimeTicks
    ) {
        for (int i = 0; i < SPIRAL_RIBBON_COUNT; i++) {
            double t = i / (double) SPIRAL_RIBBON_COUNT;
            double angle = t * Math.PI * 4.0; // 两圈螺旋
            double radius = 0.9 - t * 0.5;
            double height = t * 2.2;
            double px = origin[0] + Math.cos(angle) * radius;
            double pz = origin[2] + Math.sin(angle) * radius;
            double py = origin[1] + height;

            double vx = -Math.sin(angle) * 0.03;
            double vz = Math.cos(angle) * 0.03;
            double vy = 0.045;

            BongRibbonParticle ribbon = new BongRibbonParticle(world, px, py, pz, vx, vy, vz);
            ribbon.setRibbonWidth(0.05, 0.01);
            ribbon.setColor(r, g, b);
            ribbon.setAlphaPublic(0.75f);
            ribbon.setMaxAgePublic(lifetimeTicks);
            if (BongParticles.vortexSpiralSprites != null) {
                ribbon.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(ribbon);
        }
    }

    private static void spawnMist(
        MinecraftClient client,
        ClientWorld world,
        double[] origin,
        int lifetimeTicks
    ) {
        for (int i = 0; i < MIST_SPRITE_COUNT; i++) {
            double angle = world.random.nextDouble() * Math.PI * 2.0;
            double speed = 0.02 + world.random.nextDouble() * 0.05;
            BongSpriteParticle mist = new BongSpriteParticle(
                world,
                origin[0] + (world.random.nextDouble() - 0.5) * 1.2,
                origin[1] + world.random.nextDouble() * 1.8,
                origin[2] + (world.random.nextDouble() - 0.5) * 1.2,
                Math.cos(angle) * speed,
                0.015 + world.random.nextDouble() * 0.03,
                Math.sin(angle) * speed
            );
            mist.setColor(0.94f, 0.98f, 0.97f);
            mist.setAlphaPublic(0.5f);
            mist.setScalePublic(0.9f + (float) world.random.nextDouble() * 0.4f);
            mist.setMaxAgePublic(lifetimeTicks);
            if (BongParticles.cloudDustSprites != null) {
                mist.setSpritePublic(BongParticles.cloudDustSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(mist);
        }
    }
}
