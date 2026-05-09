package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class ChargingOrbVfx implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "charging_orb");

    private static final int FALLBACK_RGB = 0xC43CFF;
    private static final int DEFAULT_COUNT = 10;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) {
            return;
        }

        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];
        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        double strength = Math.max(0.1, Math.min(1.0, payload.strength().orElse(0.35)));
        int count = clamp(payload.count().orElse(DEFAULT_COUNT), 1, 32);
        int maxAge = payload.durationTicks().orElse(10);

        double radius = 0.35 + strength * 0.55;
        for (int i = 0; i < count; i++) {
            double angle = world.random.nextDouble() * Math.PI * 2.0;
            double height = (world.random.nextDouble() - 0.5) * 0.65;
            double px = ox + Math.cos(angle) * radius;
            double py = oy + height;
            double pz = oz + Math.sin(angle) * radius;
            double vx = (ox - px) * (0.05 + strength * 0.08);
            double vy = (world.random.nextDouble() - 0.5) * 0.025;
            double vz = (oz - pz) * (0.05 + strength * 0.08);
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
                (float) (0.35 + strength * 0.55),
                maxAge,
                (float) (0.06 + strength * 0.05)
            );
        }
    }

    private static int clamp(int value, int lo, int hi) {
        return Math.max(lo, Math.min(hi, value));
    }
}
