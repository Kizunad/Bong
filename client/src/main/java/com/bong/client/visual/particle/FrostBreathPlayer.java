package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.OptionalInt;

/**
 * {@code bong:frost_breath} —— 渊口负灵压内的寒气凝白。
 *
 * <p>复用 enlightenment_dust sprite，生成短寿命、缓慢上浮的蓝白雾粒。
 */
public final class FrostBreathPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "frost_breath");

    private static final int DEFAULT_COUNT = 4;
    private static final int FALLBACK_RGB = 0xCFEFFF;

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
        float alpha = (float) Math.max(0.25, Math.min(0.85, payload.strength().orElse(0.6)));

        int count = clamp(payload.count().orElse(OptionalInt.of(DEFAULT_COUNT).getAsInt()), 1, 32);
        int maxAge = payload.durationTicks().orElse(OptionalInt.of(20).getAsInt());

        for (int i = 0; i < count; i++) {
            double dx = (world.random.nextDouble() - 0.5) * 0.5;
            double dz = (world.random.nextDouble() - 0.5) * 0.5;
            double dy = world.random.nextDouble() * 0.25;
            double vx = (world.random.nextDouble() - 0.5) * 0.015;
            double vy = 0.015 + world.random.nextDouble() * 0.02;
            double vz = (world.random.nextDouble() - 0.5) * 0.015;
            EnlightenmentAuraPlayer.spawnSprite(
                client,
                world,
                BongParticles.enlightenmentDustSprites,
                ox + dx,
                oy + dy,
                oz + dz,
                vx,
                vy,
                vz,
                r,
                g,
                b,
                alpha,
                maxAge,
                0.06f
            );
        }
    }

    private static int clamp(int v, int lo, int hi) {
        return Math.max(lo, Math.min(hi, v));
    }
}
