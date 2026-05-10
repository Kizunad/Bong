package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.OptionalInt;

/** {@code bong:botany_aura} —— 成熟灵草周围的低频浮光。 */
public final class BotanyAuraPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "botany_aura");

    private static final int FALLBACK_RGB = 0x88CC88;
    private static final int DEFAULT_COUNT = 4;

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
        float alpha = (float) Math.max(0.35, Math.min(0.9, payload.strength().orElse(0.65)));
        int count = clamp(payload.count().orElse(OptionalInt.of(DEFAULT_COUNT).getAsInt()), 1, 12);
        int maxAgeBase = payload.durationTicks().orElse(OptionalInt.of(60).getAsInt());

        for (int i = 0; i < count; i++) {
            double angle = (Math.PI * 2.0 * i / count) + world.random.nextDouble() * 0.45;
            double radius = 0.25 + world.random.nextDouble() * 0.22;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + world.random.nextDouble() * 0.55;
            BotanyAuraParticle particle = new BotanyAuraParticle(
                world,
                x,
                y,
                z,
                0.0,
                0.012 + world.random.nextDouble() * 0.012,
                0.0,
                angle,
                radius
            );
            if (BongParticles.qiAuraSprites != null) {
                particle.setSpritePublic(BongParticles.qiAuraSprites.getSprite(world.random));
            }
            particle.setColor(r, g, b);
            particle.setAlphaPublic(alpha);
            particle.setScalePublic(0.045f + world.random.nextFloat() * 0.025f);
            particle.setMaxAgePublic(clamp(maxAgeBase + world.random.nextInt(21) - 10, 40, 80));
            client.particleManager.addParticle(particle);
        }
    }

    private static int clamp(int v, int lo, int hi) {
        return Math.max(lo, Math.min(hi, v));
    }

    private static final class BotanyAuraParticle extends BongSpriteParticle {
        private final double phase;
        private final double swayRadius;

        BotanyAuraParticle(
            ClientWorld world,
            double x,
            double y,
            double z,
            double vx,
            double vy,
            double vz,
            double phase,
            double swayRadius
        ) {
            super(world, x, y, z, vx, vy, vz);
            this.phase = phase;
            this.swayRadius = swayRadius;
        }

        @Override
        public void tick() {
            double t = (this.age + phase * 8.0) * 0.18;
            this.velocityX += Math.sin(t) * swayRadius * 0.0025;
            this.velocityZ += Math.cos(t) * swayRadius * 0.0025;
            this.velocityY *= 0.98;
            super.tick();
        }
    }
}
