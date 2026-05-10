package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class SpiderShimmerPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "spider_shimmer");

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) {
            return;
        }
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 0.3;
        double oz = payload.origin()[2];
        int count = Math.max(1, Math.min(24, payload.count().orElse(10)));
        for (int i = 0; i < count; i++) {
            double px = ox + (world.random.nextDouble() - 0.5) * 1.1;
            double pz = oz + (world.random.nextDouble() - 0.5) * 1.1;
            double py = oy + world.random.nextDouble() * 0.45;
            EnlightenmentAuraPlayer.spawnSprite(
                client,
                world,
                BongParticles.enlightenmentDustSprites,
                px,
                py,
                pz,
                0.0,
                0.012,
                0.0,
                0.72f,
                0.82f,
                0.78f,
                0.32f,
                payload.durationTicks().orElse(20),
                0.05f
            );
        }
    }
}
