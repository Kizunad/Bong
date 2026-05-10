package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

/**
 * NPC rank aura markers for elder/master silhouettes.
 */
public final class NpcRankAuraPlayer implements VfxPlayer {
    public static final Identifier ELDER = new Identifier("bong", "npc_rank_aura_elder");
    public static final Identifier MASTER = new Identifier("bong", "npc_rank_aura_master");

    private static final int FALLBACK_RGB = 0xF2D16B;
    private static final int DEFAULT_COUNT = 8;

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
        float alpha = (float) Math.max(0.25, Math.min(0.85, payload.strength().orElse(0.55)));
        int count = clamp(payload.count().orElse(DEFAULT_COUNT), 1, 24);
        int maxAge = payload.durationTicks().orElse(40);

        for (int i = 0; i < count; i++) {
            double angle = (Math.PI * 2.0 * i / count) + world.random.nextDouble() * 0.12;
            double radius = 0.45 + world.random.nextDouble() * 0.35;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + 1.0 + world.random.nextDouble() * 0.9;
            EnlightenmentAuraPlayer.spawnSprite(
                client, world, BongParticles.qiAuraSprites,
                x, y, z, 0.0, 0.018, 0.0, r, g, b, alpha, maxAge, 0.16f
            );
        }
    }

    private static int clamp(int v, int lo, int hi) {
        return Math.max(lo, Math.min(hi, v));
    }
}
