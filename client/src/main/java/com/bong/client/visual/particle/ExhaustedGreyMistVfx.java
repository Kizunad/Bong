package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class ExhaustedGreyMistVfx implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "exhausted_grey_mist");

    private static final int FALLBACK_RGB = 0x7D7782;
    private static final int DEFAULT_COUNT = 12;

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
        double strength = Math.max(0.2, Math.min(1.0, payload.strength().orElse(0.65)));
        int count = clamp(payload.count().orElse(DEFAULT_COUNT), 1, 32);
        int maxAge = payload.durationTicks().orElse(40);

        for (int i = 0; i < count; i++) {
            double px = ox + (world.random.nextDouble() - 0.5) * 0.9;
            double py = oy + world.random.nextDouble() * 0.9;
            double pz = oz + (world.random.nextDouble() - 0.5) * 0.9;
            double vx = (world.random.nextDouble() - 0.5) * 0.025;
            double vy = 0.01 + world.random.nextDouble() * 0.025;
            double vz = (world.random.nextDouble() - 0.5) * 0.025;
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
                (float) (0.2 + strength * 0.35),
                maxAge,
                0.12f
            );
        }
    }

    private static int clamp(int value, int lo, int hi) {
        return Math.max(lo, Math.min(hi, value));
    }
}
