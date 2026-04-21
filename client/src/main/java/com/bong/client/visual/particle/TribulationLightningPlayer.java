package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.OptionalInt;

/**
 * {@code bong:tribulation_lightning} —— 天劫落雷（plan §4.4）。
 *
 * <p>从 origin 上空降下 N 条 tribulation_spark Line 粒子，
 * velocity 向下 + 水平微抖，短寿命（10-16 tick）强视觉冲击。
 */
public final class TribulationLightningPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "tribulation_lightning");

    private static final int DEFAULT_COUNT = 3;
    private static final int FALLBACK_RGB = 0xD0C8FF; // 紫白
    private static final double FALL_SPEED = 3.2;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) return;

        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];

        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        float alpha = (float) Math.max(0.5, Math.min(1.0, payload.strength().orElse(1.0)));

        int count = clamp(payload.count().orElse(OptionalInt.of(DEFAULT_COUNT).getAsInt()), 1, 12);
        int maxAge = payload.durationTicks().orElse(OptionalInt.of(14).getAsInt());

        for (int i = 0; i < count; i++) {
            // 起点在上方 3-6m
            double startY = 3.0 + world.random.nextDouble() * 3.0;
            double jitterX = (world.random.nextDouble() - 0.5) * 0.8;
            double jitterZ = (world.random.nextDouble() - 0.5) * 0.8;
            // 速度主要向下，水平微偏让闪电不完全垂直
            double vy = -FALL_SPEED - world.random.nextDouble() * 0.8;
            double vx = (world.random.nextDouble() - 0.5) * 0.6;
            double vz = (world.random.nextDouble() - 0.5) * 0.6;

            BongLineParticle p = new BongLineParticle(
                world, ox + jitterX, oy + startY, oz + jitterZ, vx, vy, vz
            );
            p.setLineShape(1.0, 2.5, 0.15);
            p.setColor(r, g, b);
            p.setAlphaPublic(alpha);
            p.setMaxAgePublic(maxAge);
            if (BongParticles.tribulationSparkSprites != null) {
                p.setSpritePublic(BongParticles.tribulationSparkSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(p);
        }
    }

    private static int clamp(int v, int lo, int hi) {
        return Math.max(lo, Math.min(hi, v));
    }
}
