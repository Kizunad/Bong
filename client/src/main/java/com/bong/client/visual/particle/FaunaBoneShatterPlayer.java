package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class FaunaBoneShatterPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "fauna_bone_shatter");

    private static final int FALLBACK_RGB = 0xD8C8AA;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) {
            return;
        }

        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 0.6;
        double oz = payload.origin()[2];
        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        int count = clamp(payload.count().orElse(6), 1, 18);

        for (int i = 0; i < count; i++) {
            double theta = world.random.nextDouble() * Math.PI * 2.0;
            double speed = 0.08 + world.random.nextDouble() * 0.11;
            double vx = Math.cos(theta) * speed;
            double vz = Math.sin(theta) * speed;
            double vy = 0.04 + world.random.nextDouble() * 0.09;
            BongLineParticle shard = new BongLineParticle(world, ox, oy, oz, vx, vy, vz);
            shard.setLineShape(1.2, 0.16, 0.025);
            shard.setColor(r, g, b);
            shard.setAlphaPublic(0.78f);
            shard.setMaxAgePublic(payload.durationTicks().orElse(18));
            if (BongParticles.swordQiTrailSprites != null) {
                shard.setSpritePublic(BongParticles.swordQiTrailSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(shard);
        }
    }

    private static int clamp(int value, int lo, int hi) {
        return Math.max(lo, Math.min(hi, value));
    }
}
