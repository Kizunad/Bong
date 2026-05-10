package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.OptionalInt;

/** Spawns several inward ribbon trails around a woliu-v2 low-pressure point. */
public final class VortexSpiralPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "vortex_spiral");
    public static final Identifier VACUUM_PALM = new Identifier("bong", "woliu_vacuum_palm_spiral");
    public static final Identifier VORTEX_SHIELD = new Identifier("bong", "woliu_vortex_shield_sphere");
    public static final Identifier VACUUM_LOCK = new Identifier("bong", "woliu_vacuum_lock_cage");
    public static final Identifier VORTEX_RESONANCE = new Identifier("bong", "woliu_vortex_resonance_field");
    public static final Identifier TURBULENCE_BURST = new Identifier("bong", "woliu_turbulence_burst_wave");

    private static final int DEFAULT_COUNT = 10;
    private static final int FALLBACK_RGB = 0x201832;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) return;

        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 1.0;
        double oz = payload.origin()[2];
        int count = clamp(payload.count().orElse(OptionalInt.of(DEFAULT_COUNT).getAsInt()), 1, 32);
        int maxAge = payload.durationTicks().orElse(OptionalInt.of(42).getAsInt());
        double strength = payload.strength().orElse(0.75);
        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        float alpha = (float) Math.max(0.35, Math.min(0.95, 0.45 + strength * 0.5));

        for (int i = 0; i < count; i++) {
            double angle = (Math.PI * 2.0 * i / count) + world.random.nextDouble() * 0.35;
            double radius = 0.35 + world.random.nextDouble() * 0.65;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + (world.random.nextDouble() - 0.5) * 0.45;
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world,
                x,
                y,
                z,
                -Math.sin(angle) * 0.035,
                (world.random.nextDouble() - 0.5) * 0.012,
                Math.cos(angle) * 0.035,
                ox,
                oy,
                oz
            );
            particle.setAngularVelocity(0.055 + strength * 0.08);
            particle.setColor(r, g, b);
            particle.setAlphaPublic(alpha);
            particle.setMaxAgePublic(maxAge);
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    private static int clamp(int value, int min, int max) {
        return Math.max(min, Math.min(max, value));
    }
}
