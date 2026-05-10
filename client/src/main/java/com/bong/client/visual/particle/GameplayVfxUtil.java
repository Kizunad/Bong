package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.particle.SpriteProvider;
import net.minecraft.client.world.ClientWorld;

final class GameplayVfxUtil {
    private static final int MIN_DURATION_TICKS = 1;
    private static final int MAX_DURATION_TICKS = 600;
    private static final double[] DEFAULT_DIRECTION = new double[] { 0.0, 1.0, 0.0 };

    private GameplayVfxUtil() {
    }

    static ClientWorld world(MinecraftClient client) {
        return client == null ? null : client.world;
    }

    static float[] rgb(VfxEventPayload.SpawnParticle payload, int fallbackRgb) {
        int rgb = payload.colorRgb().orElse(fallbackRgb);
        return new float[] {
            ((rgb >> 16) & 0xFF) / 255f,
            ((rgb >> 8) & 0xFF) / 255f,
            (rgb & 0xFF) / 255f
        };
    }

    static int count(VfxEventPayload.SpawnParticle payload, int fallback, int min, int max) {
        return clamp(payload.count().orElse(fallback), min, max);
    }

    static int duration(VfxEventPayload.SpawnParticle payload, int fallback) {
        return clamp(payload.durationTicks().orElse(fallback), MIN_DURATION_TICKS, MAX_DURATION_TICKS);
    }

    static double strength(VfxEventPayload.SpawnParticle payload, double fallback) {
        return Math.max(0.0, Math.min(1.0, payload.strength().orElse(fallback)));
    }

    static void spawnSprite(
        MinecraftClient client,
        ClientWorld world,
        SpriteProvider provider,
        double x,
        double y,
        double z,
        double vx,
        double vy,
        double vz,
        float[] rgb,
        float alpha,
        int maxAge,
        float scale
    ) {
        EnlightenmentAuraPlayer.spawnSprite(
            client,
            world,
            provider,
            x,
            y,
            z,
            vx,
            vy,
            vz,
            rgb[0],
            rgb[1],
            rgb[2],
            alpha,
            maxAge,
            scale
        );
    }

    static void spawnLine(
        MinecraftClient client,
        ClientWorld world,
        SpriteProvider provider,
        double x,
        double y,
        double z,
        double vx,
        double vy,
        double vz,
        float[] rgb,
        float alpha,
        int maxAge,
        double halfWidth
    ) {
        if (client == null || world == null || provider == null || client.particleManager == null) {
            return;
        }
        BongLineParticle particle = new BongLineParticle(world, x, y, z, vx, vy, vz);
        particle.setLineShape(1.0, 0.8, halfWidth);
        particle.setSpritePublic(provider.getSprite(world.random));
        particle.setColor(rgb[0], rgb[1], rgb[2]);
        particle.setAlphaPublic(alpha);
        particle.setMaxAgePublic(maxAge);
        client.particleManager.addParticle(particle);
    }

    static void spawnDecal(
        MinecraftClient client,
        ClientWorld world,
        SpriteProvider provider,
        double x,
        double y,
        double z,
        float[] rgb,
        float alpha,
        int maxAge,
        double halfSize
    ) {
        if (client == null || world == null || provider == null || client.particleManager == null) {
            return;
        }
        BongGroundDecalParticle particle = new BongGroundDecalParticle(world, x, y, z);
        particle.setDecalShape(halfSize, 0.03);
        particle.setSpin(world.random.nextDouble() * Math.PI * 2.0, 0.035);
        particle.setSpritePublic(provider.getSprite(world.random));
        particle.setColor(rgb[0], rgb[1], rgb[2]);
        particle.setAlphaPublic(alpha);
        particle.setMaxAgePublic(maxAge);
        client.particleManager.addParticle(particle);
    }

    static double[] direction(VfxEventPayload.SpawnParticle payload, double[] fallback) {
        if (fallback == null || fallback.length < 3) {
            fallback = DEFAULT_DIRECTION;
        }
        double[] dir = payload.direction().orElse(fallback);
        if (dir == null || dir.length < 3) {
            return fallback;
        }
        double len = Math.sqrt(dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]);
        if (len <= 1e-6) {
            return fallback;
        }
        return new double[] { dir[0] / len, dir[1] / len, dir[2] / len };
    }

    static int clamp(int value, int lo, int hi) {
        return Math.max(lo, Math.min(hi, value));
    }
}
