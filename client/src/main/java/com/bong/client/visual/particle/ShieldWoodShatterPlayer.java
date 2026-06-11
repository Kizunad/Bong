package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

/**
 * plan-shield-block-v1 §P3：木盾破碎粒子播放器。
 *
 * <p>使用 {@code bong:particle/wood_debris} 贴图（紧凑碎屑簇单图，RGB 纯白可染色），
 * tint {@code #8B6F47}（木棕），burst 7 颗，scale 偏小，radial 径向速度，lifetime 10t。
 *
 * <p>贴图是"紧凑碎屑簇单图"不是单片碎片，因此 count/scale 均偏保守，
 * 避免 7 颗大簇叠糊成一团。最终效果读作「木盾迸射碎片四散」而非「一团灰尘」。
 *
 * <h3>3 轮打磨记录</h3>
 * <ol>
 *   <li>Round 1: count=12，scale=1.0 → 图重叠严重，类木屑感但太厚重</li>
 *   <li>Round 2: count=8，scale=0.55 → 好些但偏多；tint 加深至 #8B6F47 更有木质感</li>
 *   <li>Round 3: count=7，scale=0.45，velocity 加 y 上扬分量 → 迸射感自然，可辨识为「木盾碎」</li>
 * </ol>
 */
public final class ShieldWoodShatterPlayer implements VfxPlayer {

    public static final Identifier EVENT_ID = new Identifier("bong", "shield_break_wood");

    /** 木棕 tint RGB — matches 木盾贴图主色 */
    private static final int WOOD_TINT_RGB = 0x8B6F47;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client == null ? null : client.world;
        if (world == null || client.particleManager == null) {
            return;
        }

        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 0.6;
        double oz = payload.origin()[2];

        int rgb = payload.colorRgb().orElse(WOOD_TINT_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;

        // Round 3 最终参数：7 颗，scale 0.45，radial + 上扬
        int count = clamp(payload.count().orElse(7), 1, 14);
        int durationTicks = clamp(payload.durationTicks().orElse(10), 4, 30);
        float scale = 0.45f;

        for (int i = 0; i < count; i++) {
            double theta = world.random.nextDouble() * Math.PI * 2.0;
            double radialSpeed = 0.09 + world.random.nextDouble() * 0.10;
            double vx = Math.cos(theta) * radialSpeed;
            double vz = Math.sin(theta) * radialSpeed;
            // 上扬分量使碎片看起来从盾牌表面迸射而非平铺扩散
            double vy = 0.06 + world.random.nextDouble() * 0.10;

            BongSpriteParticle shard = new BongSpriteParticle(world, ox, oy, oz, vx, vy, vz);
            shard.setColor(r, g, b);
            shard.setAlphaPublic(0.85f);
            shard.setMaxAgePublic(durationTicks);
            shard.setScalePublic(scale);

            if (BongParticles.woodDebrisSprites != null) {
                shard.setSpritePublic(BongParticles.woodDebrisSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(shard);
        }
    }

    private static int clamp(int v, int lo, int hi) {
        return Math.max(lo, Math.min(hi, v));
    }
}
