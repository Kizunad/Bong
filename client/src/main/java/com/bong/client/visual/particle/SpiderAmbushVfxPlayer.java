package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

/**
 * 拟态灰烬蛛暴起粒子 — plan-fauna-mimic-spider-v1 P1。
 *
 * <p>规格（plan §P1）：
 * <ul>
 *   <li>event_id: {@code bong:vfx/spider_ambush}</li>
 *   <li>count: 16（径向 burst）</li>
 *   <li>color: {@code #B8D0C8}（灰烬蛛体色，与 FaunaVisualKind::AshSpider 一致）</li>
 *   <li>speed: ≈ 2 m/s（径向扩散）</li>
 *   <li>lifetime: 8 tick ≈ 400ms</li>
 * </ul>
 *
 * <p>复用 {@link BongParticles#ashFragmentSprites} 贴图（灰烬碎片，与蛛身材质同系）。
 */
public final class SpiderAmbushVfxPlayer implements VfxPlayer {

    /** 与 server {@code SPIDER_AMBUSH_VFX_EVENT_ID} 对齐。*/
    public static final Identifier EVENT_ID = new Identifier("bong", "vfx/spider_ambush");

    private static final int FALLBACK_RGB = 0xB8D0C8;
    private static final int DEFAULT_COUNT = 16;
    private static final int DEFAULT_DURATION_TICKS = 8;

    /** 径向扩散速度（m/tick，约 2m/s @ 20 tps）。*/
    private static final double RADIAL_SPEED = 0.1;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) {
            return;
        }

        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];

        // color from payload or fallback
        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;

        // strength drives radial speed multiplier
        double strengthMul = Math.max(0.5, Math.min(1.5,
                payload.strength().map(s -> (double) s).orElse(0.8)));
        int count = clamp(payload.count().orElse(DEFAULT_COUNT), 1, 64);
        int maxAge = payload.durationTicks().orElse(DEFAULT_DURATION_TICKS);

        for (int i = 0; i < count; i++) {
            // 径向均匀分布（水平面 burst）
            double angle = (i / (double) count) * Math.PI * 2.0
                    + world.random.nextDouble() * 0.3;
            double speed = RADIAL_SPEED * strengthMul * (0.8 + world.random.nextDouble() * 0.4);
            double vx = Math.cos(angle) * speed;
            double vz = Math.sin(angle) * speed;
            double vy = 0.02 + world.random.nextDouble() * 0.06;

            double px = ox + world.random.nextDouble() * 0.3 - 0.15;
            double py = oy + world.random.nextDouble() * 0.3;
            double pz = oz + world.random.nextDouble() * 0.3 - 0.15;

            // 复用 qiAuraSprites（BongSpriteParticle 精灵粒子，与蛛身灰烬质感一致）
            EnlightenmentAuraPlayer.spawnSprite(
                    client,
                    world,
                    BongParticles.qiAuraSprites,
                    px, py, pz,
                    vx, vy, vz,
                    r, g, b,
                    0.55f,
                    maxAge,
                    0.08f
            );
        }
    }

    private static int clamp(int value, int lo, int hi) {
        return Math.max(lo, Math.min(hi, value));
    }
}
