package com.bong.client.environment;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.particle.ParticleTypes;
import net.minecraft.util.math.Vec3d;
import net.minecraft.util.math.random.Random;

public final class EnvironmentParticleHelper {
    private EnvironmentParticleHelper() {
    }

    public static void spawn(EnvironmentEffect effect, float alpha, float tickDelta) {
        MinecraftClient client = MinecraftClient.getInstance();
        ClientWorld world = client == null ? null : client.world;
        if (world == null || effect == null || alpha <= 0.0f) {
            return;
        }

        long seed = world.getTime() ^ effect.stableKey().hashCode();
        Random random = Random.create(seed);
        int repeats = Math.max(1, Math.round(alpha * 3.0f));
        for (int i = 0; i < repeats; i++) {
            if (effect instanceof EnvironmentEffect.TornadoColumn tornado) {
                spawnTornado(world, tornado, random, alpha, tickDelta);
            } else if (effect instanceof EnvironmentEffect.LightningPillar lightning) {
                spawnLightning(world, lightning, random, alpha);
            } else if (effect instanceof EnvironmentEffect.AshFall ash) {
                spawnAsh(world, ash, random, alpha);
            } else if (effect instanceof EnvironmentEffect.FogVeil fog) {
                spawnFog(world, fog, random, alpha);
            } else if (effect instanceof EnvironmentEffect.DustDevil dust) {
                spawnDust(world, dust, random, alpha);
            } else if (effect instanceof EnvironmentEffect.EmberDrift ember) {
                spawnEmber(world, ember, random, alpha);
            } else if (effect instanceof EnvironmentEffect.HeatHaze haze) {
                spawnHeatHaze(world, haze, random, alpha);
            } else if (effect instanceof EnvironmentEffect.SnowDrift snow) {
                spawnSnow(world, snow, random, alpha);
            }
        }
    }

    private static void spawnTornado(
        ClientWorld world,
        EnvironmentEffect.TornadoColumn effect,
        Random random,
        float alpha,
        float tickDelta
    ) {
        double angle = (world.getTime() + tickDelta) * 0.25 + random.nextDouble() * Math.PI;
        double layer = random.nextDouble() * effect.height();
        double radius = effect.radius() * (0.4 + random.nextDouble() * 0.6);
        double x = effect.centerX() + Math.cos(angle) * radius;
        double z = effect.centerZ() + Math.sin(angle) * radius;
        double y = effect.centerY() + layer;
        world.addParticle(ParticleTypes.CLOUD, x, y, z, 0.0, 0.02 * alpha, 0.0);
        world.addParticle(ParticleTypes.LARGE_SMOKE, x, y + 0.5, z, 0.0, 0.01 * alpha, 0.0);
    }

    private static void spawnLightning(
        ClientWorld world,
        EnvironmentEffect.LightningPillar effect,
        Random random,
        float alpha
    ) {
        double x = effect.centerX() + (random.nextDouble() - 0.5) * effect.radius();
        double z = effect.centerZ() + (random.nextDouble() - 0.5) * effect.radius();
        double y = effect.centerY() + random.nextDouble() * 10.0;
        world.addParticle(ParticleTypes.ELECTRIC_SPARK, x, y, z, 0.0, 0.2 * alpha, 0.0);
        world.addParticle(ParticleTypes.FLAME, x, y - 0.25, z, 0.0, 0.05 * alpha, 0.0);
    }

    private static void spawnAsh(
        ClientWorld world,
        EnvironmentEffect.AshFall effect,
        Random random,
        float alpha
    ) {
        double x = lerp(effect.minX(), effect.maxX(), random.nextDouble());
        double y = effect.maxY() - random.nextDouble() * 6.0;
        double z = lerp(effect.minZ(), effect.maxZ(), random.nextDouble());
        world.addParticle(ParticleTypes.SMOKE, x, y, z, 0.0, -0.015 * alpha, 0.0);
    }

    private static void spawnFog(
        ClientWorld world,
        EnvironmentEffect.FogVeil effect,
        Random random,
        float alpha
    ) {
        double x = lerp(effect.minX(), effect.maxX(), random.nextDouble());
        double y = lerp(effect.minY(), effect.maxY(), random.nextDouble());
        double z = lerp(effect.minZ(), effect.maxZ(), random.nextDouble());
        world.addParticle(ParticleTypes.CLOUD, x, y, z, 0.0, 0.005 * alpha, 0.0);
    }

    private static void spawnDust(
        ClientWorld world,
        EnvironmentEffect.DustDevil effect,
        Random random,
        float alpha
    ) {
        double angle = (world.getTime() * 0.4) + random.nextDouble() * Math.PI * 2.0;
        double radius = effect.radius() * (0.2 + random.nextDouble() * 0.8);
        double x = effect.centerX() + Math.cos(angle) * radius;
        double z = effect.centerZ() + Math.sin(angle) * radius;
        double y = effect.centerY() + random.nextDouble() * effect.height();
        world.addParticle(ParticleTypes.LARGE_SMOKE, x, y, z, 0.0, 0.015 * alpha, 0.0);
    }

    private static void spawnEmber(
        ClientWorld world,
        EnvironmentEffect.EmberDrift effect,
        Random random,
        float alpha
    ) {
        double x = lerp(effect.minX(), effect.maxX(), random.nextDouble());
        double y = lerp(effect.minY(), effect.maxY(), random.nextDouble());
        double z = lerp(effect.minZ(), effect.maxZ(), random.nextDouble());
        world.addParticle(ParticleTypes.FLAME, x, y, z, 0.0, 0.03 * alpha, 0.0);
    }

    private static void spawnHeatHaze(
        ClientWorld world,
        EnvironmentEffect.HeatHaze effect,
        Random random,
        float alpha
    ) {
        double x = lerp(effect.minX(), effect.maxX(), random.nextDouble());
        double y = lerp(effect.minY(), effect.maxY(), random.nextDouble());
        double z = lerp(effect.minZ(), effect.maxZ(), random.nextDouble());
        world.addParticle(ParticleTypes.SMOKE, x, y, z, 0.0, 0.002 * alpha, 0.0);
    }

    private static void spawnSnow(
        ClientWorld world,
        EnvironmentEffect.SnowDrift effect,
        Random random,
        float alpha
    ) {
        double x = lerp(effect.minX(), effect.maxX(), random.nextDouble());
        double y = lerp(effect.minY(), effect.maxY(), random.nextDouble());
        double z = lerp(effect.minZ(), effect.maxZ(), random.nextDouble());
        world.addParticle(ParticleTypes.SNOWFLAKE, x, y, z, effect.windX() * 0.015 * alpha, -0.02 * alpha, effect.windZ() * 0.015 * alpha);
    }

    private static double lerp(double min, double max, double t) {
        return min + (max - min) * t;
    }
}
