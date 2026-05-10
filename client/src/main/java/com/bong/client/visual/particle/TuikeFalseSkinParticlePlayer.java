package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

/** Particle players for tuike-v2 false-skin skills. */
public final class TuikeFalseSkinParticlePlayer implements VfxPlayer {
    public static final Identifier DON_DUST = new Identifier("bong", "false_skin_don_dust");
    public static final Identifier SHED_BURST = new Identifier("bong", "false_skin_shed_burst");
    public static final Identifier ANCIENT_GLOW = new Identifier("bong", "ancient_skin_glow");
    private static final int MAX_AGE_TICKS = 200;

    private final Identifier eventId;

    public TuikeFalseSkinParticlePlayer(Identifier eventId) {
        this.eventId = eventId;
    }

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) return;

        double[] origin = payload.origin();
        if (origin.length < 3) return;

        double ox = origin[0];
        double oy = origin[1];
        double oz = origin[2];
        int count = clamp(payload.count().orElse(defaultCount()), 1, 48);
        int maxAge = clamp(payload.durationTicks().orElse(defaultAge()), 1, MAX_AGE_TICKS);
        double strength = Math.max(0.2, Math.min(1.0, payload.strength().orElse(0.75)));
        int rgb = payload.colorRgb().orElse(defaultRgb());
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;

        for (int i = 0; i < count; i++) {
            double angle = Math.PI * 2.0 * i / count + world.random.nextDouble() * 0.2;
            double radius = spreadRadius(strength) * world.random.nextDouble();
            double px = ox + Math.cos(angle) * radius;
            double py = oy + world.random.nextDouble() * 1.15;
            double pz = oz + Math.sin(angle) * radius;
            double vx = Math.cos(angle) * horizontalSpeed(strength);
            double vy = verticalSpeed(strength);
            double vz = Math.sin(angle) * horizontalSpeed(strength);

            EnlightenmentAuraPlayer.spawnSprite(
                client,
                world,
                spriteProvider(),
                px,
                py,
                pz,
                vx,
                vy,
                vz,
                r,
                g,
                b,
                (float) (0.28 + strength * 0.45),
                maxAge,
                particleScale(strength)
            );
        }
    }

    private net.minecraft.client.particle.SpriteProvider spriteProvider() {
        if (ANCIENT_GLOW.equals(eventId)) return BongParticles.enlightenmentDustSprites;
        if (SHED_BURST.equals(eventId)) return BongParticles.tribulationSparkSprites;
        return BongParticles.qiAuraSprites;
    }

    private int defaultCount() {
        if (SHED_BURST.equals(eventId)) return 18;
        if (ANCIENT_GLOW.equals(eventId)) return 16;
        return 10;
    }

    private int defaultAge() {
        return ANCIENT_GLOW.equals(eventId) ? 48 : 34;
    }

    private int defaultRgb() {
        if (ANCIENT_GLOW.equals(eventId)) return 0xBFD8FF;
        if (SHED_BURST.equals(eventId)) return 0xB58B5A;
        return 0xD8C08A;
    }

    private double spreadRadius(double strength) {
        if (SHED_BURST.equals(eventId)) return 0.9 + strength * 0.45;
        return 0.35 + strength * 0.35;
    }

    private double horizontalSpeed(double strength) {
        if (SHED_BURST.equals(eventId)) return 0.025 + strength * 0.035;
        return 0.006 + strength * 0.012;
    }

    private double verticalSpeed(double strength) {
        if (ANCIENT_GLOW.equals(eventId)) return 0.018 + strength * 0.02;
        if (SHED_BURST.equals(eventId)) return 0.008 + strength * 0.02;
        return 0.012 + strength * 0.018;
    }

    private float particleScale(double strength) {
        if (ANCIENT_GLOW.equals(eventId)) return (float) (0.12 + strength * 0.12);
        return (float) (0.08 + strength * 0.08);
    }

    private static int clamp(int value, int min, int max) {
        return Math.max(min, Math.min(max, value));
    }
}
