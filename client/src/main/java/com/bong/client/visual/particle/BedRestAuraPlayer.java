package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class BedRestAuraPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "furniture_bed_rest");

    private static final int FALLBACK_RGB = 0xE8C97A;
    private static final int DEFAULT_COUNT = 6;
    private static final int DEFAULT_MAX_AGE = 12;

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
        float alpha = (float) Math.max(0.45, Math.min(0.9, payload.strength().orElse(0.75)));
        int count = clamp(payload.count().orElse(DEFAULT_COUNT), 1, 16);
        int maxAge = clamp(payload.durationTicks().orElse(DEFAULT_MAX_AGE), 1, 40);

        for (int i = 0; i < count; i++) {
            double dx = (world.random.nextDouble() - 0.5) * 0.9;
            double dy = 0.2 + world.random.nextDouble() * 0.7;
            double dz = (world.random.nextDouble() - 0.5) * 0.9;
            double vx = (world.random.nextDouble() - 0.5) * 0.025;
            double vy = 0.018 + world.random.nextDouble() * 0.018;
            double vz = (world.random.nextDouble() - 0.5) * 0.025;
            EnlightenmentAuraPlayer.spawnSprite(client, world, BongParticles.enlightenmentDustSprites,
                ox + dx, oy + dy, oz + dz, vx, vy, vz, r, g, b, alpha, maxAge, 0.06f);
        }
    }

    private static int clamp(int value, int min, int max) {
        return Math.max(min, Math.min(max, value));
    }
}
