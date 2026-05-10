package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

/**
 * Grey residual smoke left by an NPC death after the soul-dissipate burst.
 */
public final class NpcDeathSmokePlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "npc_death_smoke");

    private static final int FALLBACK_RGB = 0xB8B8B8;
    private static final int DEFAULT_COUNT = 16;

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
        float alpha = (float) Math.max(0.2, Math.min(0.7, payload.strength().orElse(0.45)));
        int count = clamp(payload.count().orElse(DEFAULT_COUNT), 1, 36);
        int maxAge = payload.durationTicks().orElse(60);

        for (int i = 0; i < count; i++) {
            double dx = (world.random.nextDouble() - 0.5) * 0.8;
            double dz = (world.random.nextDouble() - 0.5) * 0.8;
            double vx = dx * 0.015;
            double vz = dz * 0.015;
            double vy = 0.018 + world.random.nextDouble() * 0.025;
            EnlightenmentAuraPlayer.spawnSprite(
                client, world, BongParticles.qiAuraSprites,
                ox + dx, oy + 0.25 + world.random.nextDouble() * 0.35, oz + dz,
                vx, vy, vz, r, g, b, alpha, maxAge, 0.22f
            );
        }
    }

    private static int clamp(int v, int lo, int hi) {
        return Math.max(lo, Math.min(hi, v));
    }
}
