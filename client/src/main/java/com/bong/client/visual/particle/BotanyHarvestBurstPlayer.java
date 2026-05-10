package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

/** {@code bong:botany_harvest} —— 采收瞬间的碎叶与稀有光柱。 */
public final class BotanyHarvestBurstPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "botany_harvest");

    private static final int FALLBACK_RGB = 0x88CC55;
    private static final int DEFAULT_COUNT = 12;

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
        float alpha = (float) Math.max(0.5, Math.min(1.0, payload.strength().orElse(0.85)));
        int count = clamp(payload.count().orElse(DEFAULT_COUNT), 1, 32);
        int maxAge = payload.durationTicks().orElse(36);

        for (int i = 0; i < count; i++) {
            double angle = world.random.nextDouble() * Math.PI * 2.0;
            double speed = 0.045 + world.random.nextDouble() * 0.055;
            double vx = Math.cos(angle) * speed;
            double vz = Math.sin(angle) * speed;
            double vy = 0.055 + world.random.nextDouble() * 0.065;
            EnlightenmentAuraPlayer.spawnSprite(
                client,
                world,
                BongParticles.enlightenmentDustSprites,
                ox + (world.random.nextDouble() - 0.5) * 0.25,
                oy + world.random.nextDouble() * 0.35,
                oz + (world.random.nextDouble() - 0.5) * 0.25,
                vx,
                vy,
                vz,
                r,
                g,
                b,
                alpha,
                maxAge,
                0.055f + world.random.nextFloat() * 0.03f
            );
        }

        if (payload.strength().orElse(0.0) >= 0.9) {
            new BreakthroughPillarPlayer().play(
                client,
                new VfxEventPayload.SpawnParticle(
                    BreakthroughPillarPlayer.EVENT_ID,
                    new double[] { ox, oy, oz },
                    java.util.Optional.empty(),
                    java.util.OptionalInt.of(0xFFDD66),
                    java.util.Optional.of(0.55),
                    java.util.OptionalInt.of(4),
                    java.util.OptionalInt.of(20)
                )
            );
        }
    }

    private static int clamp(int v, int lo, int hi) {
        return Math.max(lo, Math.min(hi, v));
    }
}
