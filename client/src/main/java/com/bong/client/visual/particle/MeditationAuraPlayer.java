package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class MeditationAuraPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "furniture_meditation");

    private static final int FALLBACK_RGB = 0xBFD8C8;
    private static final int DEFAULT_COUNT = 8;
    private static final int DEFAULT_MAX_AGE = 16;

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
        float alpha = (float) Math.max(0.4, Math.min(0.85, payload.strength().orElse(0.75)));
        int count = clamp(payload.count().orElse(DEFAULT_COUNT), 1, 24);
        int maxAge = clamp(payload.durationTicks().orElse(DEFAULT_MAX_AGE), 1, 48);
        double phase = world.random.nextDouble() * Math.PI * 2.0;

        for (int i = 0; i < count; i++) {
            double angle = phase + (Math.PI * 2.0 * i / count);
            double radius = 0.45 + world.random.nextDouble() * 0.18;
            double x = ox + Math.cos(angle) * radius;
            double y = oy + 0.25 + world.random.nextDouble() * 0.55;
            double z = oz + Math.sin(angle) * radius;
            double vx = -Math.sin(angle) * 0.012;
            double vy = 0.008 + world.random.nextDouble() * 0.012;
            double vz = Math.cos(angle) * 0.012;
            EnlightenmentAuraPlayer.spawnSprite(client, world, BongParticles.enlightenmentDustSprites,
                x, y, z, vx, vy, vz, r, g, b, alpha, maxAge, 0.065f);
        }
    }

    private static int clamp(int value, int min, int max) {
        return Math.max(min, Math.min(max, value));
    }
}
