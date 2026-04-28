package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;

import java.util.OptionalInt;

/** {@code bong:burst_meridian_beng_quan}：崩拳短距古铜色真元爆发。 */
public final class BurstMeridianBengQuanPlayer implements VfxPlayer {
    public static final net.minecraft.util.Identifier EVENT_ID =
        new net.minecraft.util.Identifier("bong", "burst_meridian_beng_quan");

    private static final int FALLBACK_RGB = 0xC58B3F;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) {
            return;
        }

        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];
        double[] dir = payload.direction().orElse(new double[] { 1.0, 0.0, 0.0 });
        double len = Math.sqrt(dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]);
        if (len <= 1e-6) {
            dir = new double[] { 1.0, 0.0, 0.0 };
            len = 1.0;
        }
        double dx = dir[0] / len;
        double dy = dir[1] / len;
        double dz = dir[2] / len;

        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        double strength = payload.strength().orElse(0.9);
        int count = clamp(payload.count().orElse(OptionalInt.of(8).getAsInt()), 1, 24);
        int maxAge = payload.durationTicks().orElse(OptionalInt.of(8).getAsInt());

        for (int i = 0; i < count; i++) {
            double t = count == 1 ? 0.0 : (double) i / (count - 1);
            double spread = (t - 0.5) * 0.6;
            double px = ox + dx * t * 0.7 + spread * dz;
            double py = oy + 0.08 * Math.sin(t * Math.PI);
            double pz = oz + dz * t * 0.7 - spread * dx;

            BongLineParticle particle = new BongLineParticle(
                world,
                px,
                py,
                pz,
                dx * (0.55 + 0.25 * strength),
                dy * 0.25,
                dz * (0.55 + 0.25 * strength)
            );
            particle.setLineShape(1.1, 0.7, 0.22 + 0.18 * strength);
            particle.setColor(r, g, b);
            particle.setAlphaPublic((float) Math.max(0.25, Math.min(1.0, strength)));
            particle.setMaxAgePublic(maxAge);
            if (BongParticles.qiAuraSprites != null) {
                particle.setSpritePublic(BongParticles.qiAuraSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    private static int clamp(int v, int lo, int hi) {
        return Math.max(lo, Math.min(hi, v));
    }
}
