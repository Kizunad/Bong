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

        if (VORTEX_RESONANCE.equals(payload.eventId())) {
            playResonanceField(client, world, payload);
            return;
        }
        if (TURBULENCE_BURST.equals(payload.eventId())) {
            playTurbulenceBurst(client, world, payload);
            return;
        }

        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 1.0;
        double oz = payload.origin()[2];
        int count = clamp(payload.count().orElse(OptionalInt.of(DEFAULT_COUNT).getAsInt()), 1, 64);
        int maxAge = payload.durationTicks().orElse(OptionalInt.of(42).getAsInt());
        double strength = clamp01(payload.strength().orElse(0.75));
        float[] color = rgb(payload);
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
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic(alpha);
            particle.setMaxAgePublic(maxAge);
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    private static void playResonanceField(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload
    ) {
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 0.95;
        double oz = payload.origin()[2];
        double strength = clamp01(payload.strength().orElse(0.8));
        int count = clamp(payload.count().orElse(48), 24, 96);
        int maxAge = clamp(payload.durationTicks().orElse(80), 30, 120);
        double fieldRadius = 2.2 + strength * 3.8;
        float[] color = rgb(payload);

        for (int i = 0; i < count; i++) {
            int ring = i % 3;
            double ringRatio = 0.34 + ring * 0.28;
            double angle = Math.PI * 2.0 * i / count + world.random.nextDouble() * 0.22;
            double radius = fieldRadius * ringRatio + (world.random.nextDouble() - 0.5) * 0.35;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + Math.sin(angle * 2.0 + ring) * 0.32 + (world.random.nextDouble() - 0.5) * 0.18;
            double tangent = 0.055 + strength * 0.045 + ring * 0.012;
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world,
                x,
                y,
                z,
                -Math.sin(angle) * tangent,
                (world.random.nextDouble() - 0.5) * 0.012,
                Math.cos(angle) * tangent,
                ox,
                oy,
                oz
            );
            particle.setAngularVelocity(0.09 + strength * 0.09 + ring * 0.015);
            particle.setRibbonWidth(0.12 + strength * 0.05, 0.018);
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic((float) Math.min(0.9, 0.48 + strength * 0.34));
            particle.setMaxAgePublic(maxAge - world.random.nextInt(Math.max(1, maxAge / 4)));
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    private static void playTurbulenceBurst(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload
    ) {
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 0.75;
        double oz = payload.origin()[2];
        double strength = clamp01(payload.strength().orElse(0.9));
        int count = clamp(payload.count().orElse(64), 24, 96);
        int maxAge = clamp(payload.durationTicks().orElse(44), 18, 80);
        double radius = 0.6 + strength * 0.7;
        float[] color = rgb(payload);

        for (int i = 0; i < count; i++) {
            double angle = Math.PI * 2.0 * i / count + world.random.nextDouble() * 0.16;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + (world.random.nextDouble() - 0.5) * 0.5;
            double speed = 0.10 + strength * 0.08 + world.random.nextDouble() * 0.04;
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world,
                x,
                y,
                z,
                Math.cos(angle) * speed,
                (world.random.nextDouble() - 0.2) * 0.025,
                Math.sin(angle) * speed,
                ox,
                oy,
                oz
            );
            particle.setAngularVelocity(0.02 + strength * 0.04);
            particle.setRibbonWidth(0.14 + strength * 0.04, 0.02);
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic((float) Math.min(0.92, 0.55 + strength * 0.32));
            particle.setMaxAgePublic(maxAge - world.random.nextInt(Math.max(1, maxAge / 3)));
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    private static float[] rgb(VfxEventPayload.SpawnParticle payload) {
        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        return new float[] {
            ((rgb >> 16) & 0xFF) / 255f,
            ((rgb >> 8) & 0xFF) / 255f,
            (rgb & 0xFF) / 255f
        };
    }

    private static int clamp(int value, int min, int max) {
        return Math.max(min, Math.min(max, value));
    }

    private static double clamp01(double value) {
        return Math.max(0.0, Math.min(1.0, value));
    }
}
