package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class ReleaseLightningVfx implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "release_lightning");

    private static final int FALLBACK_RGB = 0xB445FF;
    private static final int DEFAULT_COUNT = 18;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) {
            return;
        }

        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];
        double[] dir = payload.direction().orElse(new double[] { 0.0, 0.0, 1.0 });
        double len = Math.sqrt(dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]);
        if (len <= 1e-6) {
            dir = new double[] { 0.0, 0.0, 1.0 };
            len = 1.0;
        }
        double dx = dir[0] / len;
        double dy = dir[1] / len;
        double dz = dir[2] / len;

        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        double strength = Math.max(0.4, Math.min(1.0, payload.strength().orElse(1.0)));
        int count = clamp(payload.count().orElse(DEFAULT_COUNT), 1, 48);
        int maxAge = payload.durationTicks().orElse(10);

        double span = Math.min(len, 8.0);
        for (int i = 0; i < count; i++) {
            double t = count == 1 ? 0.0 : (double) i / (count - 1);
            double jitter = (world.random.nextDouble() - 0.5) * 0.45;
            double px = ox + dx * span * t + jitter * dz;
            double py = oy + dy * span * t + (world.random.nextDouble() - 0.5) * 0.25;
            double pz = oz + dz * span * t - jitter * dx;

            BongLineParticle particle = new BongLineParticle(
                world,
                px,
                py,
                pz,
                dx * (1.2 + strength),
                dy * (0.8 + strength * 0.4),
                dz * (1.2 + strength)
            );
            particle.setLineShape(1.2, 1.0, 0.14 + 0.1 * strength);
            particle.setColor(r, g, b);
            particle.setAlphaPublic((float) (0.45 + strength * 0.5));
            particle.setMaxAgePublic(maxAge);
            if (BongParticles.tribulationSparkSprites != null) {
                particle.setSpritePublic(BongParticles.tribulationSparkSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    private static int clamp(int value, int lo, int hi) {
        return Math.max(lo, Math.min(hi, value));
    }
}
