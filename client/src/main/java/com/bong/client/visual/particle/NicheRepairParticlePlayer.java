package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class NicheRepairParticlePlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "social_niche_repair");

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }
        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];
        float[] rgb = GameplayVfxUtil.rgb(payload, 0xB8B0A0);
        int count = GameplayVfxUtil.count(payload, 6, 1, 16);
        int maxAge = GameplayVfxUtil.duration(payload, 14);
        for (int i = 0; i < count; i++) {
            double angle = world.random.nextDouble() * Math.PI * 2.0;
            double radius = 0.18 + world.random.nextDouble() * 0.42;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + 0.08 + world.random.nextDouble() * 0.38;
            GameplayVfxUtil.spawnSprite(
                client,
                world,
                BongParticles.enlightenmentDustSprites,
                x,
                y,
                z,
                (ox - x) * 0.018,
                0.035 + world.random.nextDouble() * 0.02,
                (oz - z) * 0.018,
                rgb,
                0.78f,
                maxAge,
                0.045f + world.random.nextFloat() * 0.025f
            );
        }
    }
}
