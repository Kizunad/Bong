package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.OptionalInt;

/**
 * {@code bong:tribulation_omen_cloud} —— 渡虚劫预兆雷云。
 *
 * <p>服务端用 direction 携带 [halfX, halfY, halfZ]，客户端在上空盒体中生成灰紫云团。
 */
public final class TribulationOmenCloudPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "tribulation_omen_cloud");

    private static final int FALLBACK_RGB = 0x3B3448;
    private static final int DEFAULT_COUNT = 36;
    private static final int DEFAULT_DURATION_TICKS = 200;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) return;

        double[] origin = payload.origin();
        double[] extent = payload.direction().orElse(new double[] { 24.0, 8.0, 24.0 });
        double halfX = Math.max(2.0, Math.abs(extent[0]));
        double halfY = Math.max(1.0, Math.abs(extent[1]));
        double halfZ = Math.max(2.0, Math.abs(extent[2]));
        int count = clamp(payload.count().orElse(DEFAULT_COUNT), 4, 64);
        int maxAge = payload.durationTicks().orElse(OptionalInt.of(DEFAULT_DURATION_TICKS).getAsInt());
        double strength = clamp(payload.strength().orElse(0.85), 0.0, 1.0);

        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        float alpha = (float) (0.28 + strength * 0.35);

        for (int i = 0; i < count; i++) {
            double x = origin[0] + (world.random.nextDouble() * 2.0 - 1.0) * halfX;
            double y = origin[1] + (world.random.nextDouble() * 2.0 - 1.0) * halfY;
            double z = origin[2] + (world.random.nextDouble() * 2.0 - 1.0) * halfZ;
            double swirl = 0.006 + world.random.nextDouble() * 0.012;
            double vx = (origin[2] - z) * swirl;
            double vz = (x - origin[0]) * swirl;

            BongSpriteParticle particle = new BongSpriteParticle(world, x, y, z, vx, -0.004, vz);
            particle.setColor(r, g, b);
            particle.setAlphaPublic(alpha);
            particle.setMaxAgePublic(maxAge - world.random.nextInt(Math.max(1, Math.min(maxAge / 3, 40))));
            if (BongParticles.qiAuraSprites != null) {
                particle.setSpritePublic(BongParticles.qiAuraSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    private static double clamp(double value, double min, double max) {
        return Math.max(min, Math.min(max, value));
    }

    private static int clamp(int value, int min, int max) {
        return Math.max(min, Math.min(max, value));
    }
}
