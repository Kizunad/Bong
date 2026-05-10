package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class RatSwarmAuraPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "rat_swarm_aura");

    private static final int FALLBACK_RGB = 0xFF4444;
    private static final int DEFAULT_COUNT = 24;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) {
            return;
        }

        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 0.35;
        double oz = payload.origin()[2];
        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        double strength = Math.max(0.25, Math.min(1.0, payload.strength().orElse(0.8)));
        int count = clamp(payload.count().orElse(DEFAULT_COUNT), 1, 64);
        int maxAge = payload.durationTicks().orElse(32);

        for (int i = 0; i < count; i++) {
            double angle = world.random.nextDouble() * Math.PI * 2.0;
            double radius = 0.9 + world.random.nextDouble() * 2.2;
            double px = ox + Math.cos(angle) * radius;
            double pz = oz + Math.sin(angle) * radius;
            double py = oy + (world.random.nextDouble() - 0.5) * 0.5;
            double vx = (ox - px) * (0.045 + 0.025 * strength);
            double vy = (oy - py) * 0.035 + 0.01;
            double vz = (oz - pz) * (0.045 + 0.025 * strength);
            EnlightenmentAuraPlayer.spawnSprite(
                client,
                world,
                BongParticles.qiAuraSprites,
                px,
                py,
                pz,
                vx,
                vy,
                vz,
                r,
                g,
                b,
                (float) (0.35 + 0.45 * strength),
                maxAge,
                0.16f
            );
        }
    }

    private static int clamp(int value, int lo, int hi) {
        return Math.max(lo, Math.min(hi, value));
    }
}
