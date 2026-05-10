package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class FaunaSpawnDustPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "fauna_spawn_dust");

    private static final int FALLBACK_RGB = 0x9A8A74;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) {
            return;
        }

        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 0.05;
        double oz = payload.origin()[2];
        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        int count = clamp(payload.count().orElse(8), 1, 32);

        for (int i = 0; i < count; i++) {
            double angle = world.random.nextDouble() * Math.PI * 2.0;
            double speed = 0.04 + world.random.nextDouble() * 0.055;
            double vx = Math.cos(angle) * speed;
            double vz = Math.sin(angle) * speed;
            double vy = 0.035 + world.random.nextDouble() * 0.045;
            EnlightenmentAuraPlayer.spawnSprite(
                client,
                world,
                BongParticles.enlightenmentDustSprites,
                ox,
                oy,
                oz,
                vx,
                vy,
                vz,
                r,
                g,
                b,
                0.55f,
                payload.durationTicks().orElse(24),
                0.08f
            );
        }
    }

    private static int clamp(int value, int lo, int hi) {
        return Math.max(lo, Math.min(hi, value));
    }
}
