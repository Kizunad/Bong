package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class CalamityVfxPlayer implements VfxPlayer {
    public static final Identifier THUNDER = new Identifier("bong", "calamity_thunder");
    public static final Identifier MIASMA = new Identifier("bong", "calamity_miasma");
    public static final Identifier MERIDIAN_SEAL = new Identifier("bong", "calamity_meridian_seal");
    public static final Identifier DAOXIANG_WAVE = new Identifier("bong", "calamity_daoxiang_wave");
    public static final Identifier HEAVENLY_FIRE = new Identifier("bong", "calamity_heavenly_fire");
    public static final Identifier PRESSURE_INVERT = new Identifier("bong", "calamity_pressure_invert");
    public static final Identifier ALL_WITHER = new Identifier("bong", "calamity_all_wither");

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }
        String id = payload.eventId().toString();
        if (THUNDER.toString().equals(id)) {
            playThunder(client, world, payload);
        } else if (MIASMA.toString().equals(id)) {
            playCloud(client, world, payload, 0x305020, 0.035, 0.02);
        } else if (MERIDIAN_SEAL.toString().equals(id)) {
            playRing(client, world, payload, 0xA0C8D8, 0.10);
        } else if (DAOXIANG_WAVE.toString().equals(id)) {
            playBurst(client, world, payload, 0xC0B090, 0.08);
        } else if (HEAVENLY_FIRE.toString().equals(id)) {
            playFire(client, world, payload);
        } else if (PRESSURE_INVERT.toString().equals(id)) {
            playRing(client, world, payload, 0x102040, -0.08);
        } else if (ALL_WITHER.toString().equals(id)) {
            playWither(client, world, payload);
        }
    }

    private static void playThunder(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload
    ) {
        double[] origin = payload.origin();
        float[] rgb = GameplayVfxUtil.rgb(payload, 0xE0E8FF);
        int count = GameplayVfxUtil.count(payload, 3, 1, 8);
        int age = GameplayVfxUtil.duration(payload, 14);
        for (int i = 0; i < count; i++) {
            GameplayVfxUtil.spawnLine(client, world, BongParticles.tribulationSparkSprites,
                origin[0] + (world.random.nextDouble() - 0.5) * 6.0,
                origin[1] + 12.0 + world.random.nextDouble() * 8.0,
                origin[2] + (world.random.nextDouble() - 0.5) * 6.0,
                (world.random.nextDouble() - 0.5) * 0.5,
                -3.0 - world.random.nextDouble(),
                (world.random.nextDouble() - 0.5) * 0.5,
                rgb, 0.9f, age, 0.18);
        }
    }

    private static void playCloud(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        int fallbackRgb,
        double speed,
        double rise
    ) {
        double[] origin = payload.origin();
        double radius = radius(payload, 8.0);
        float[] rgb = GameplayVfxUtil.rgb(payload, fallbackRgb);
        int count = GameplayVfxUtil.count(payload, 18, 4, 48);
        int age = GameplayVfxUtil.duration(payload, 80);
        for (int i = 0; i < count; i++) {
            double angle = world.random.nextDouble() * Math.PI * 2.0;
            double dist = world.random.nextDouble() * radius;
            GameplayVfxUtil.spawnSprite(client, world, BongParticles.qiAuraSprites,
                origin[0] + Math.cos(angle) * dist,
                origin[1] + world.random.nextDouble() * 3.0,
                origin[2] + Math.sin(angle) * dist,
                Math.cos(angle) * speed,
                rise + world.random.nextDouble() * rise,
                Math.sin(angle) * speed,
                rgb, 0.35f, age, 0.28f);
        }
    }

    private static void playRing(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        int fallbackRgb,
        double vertical
    ) {
        double[] origin = payload.origin();
        double radius = radius(payload, 10.0);
        float[] rgb = GameplayVfxUtil.rgb(payload, fallbackRgb);
        int count = GameplayVfxUtil.count(payload, 16, 4, 48);
        int age = GameplayVfxUtil.duration(payload, 80);
        GameplayVfxUtil.spawnDecal(client, world, BongParticles.qiAuraSprites,
            origin[0], origin[1] + 0.05, origin[2], rgb, 0.32f, age, radius * 0.5);
        for (int i = 0; i < count; i++) {
            double angle = i * Math.PI * 2.0 / count;
            GameplayVfxUtil.spawnLine(client, world, BongParticles.qiAuraSprites,
                origin[0] + Math.cos(angle) * radius,
                origin[1] + 0.2,
                origin[2] + Math.sin(angle) * radius,
                -Math.sin(angle) * 0.04,
                vertical,
                Math.cos(angle) * 0.04,
                rgb, 0.55f, age, 0.08);
        }
    }

    private static void playBurst(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        int fallbackRgb,
        double speed
    ) {
        double[] origin = payload.origin();
        float[] rgb = GameplayVfxUtil.rgb(payload, fallbackRgb);
        int count = GameplayVfxUtil.count(payload, 12, 4, 36);
        int age = GameplayVfxUtil.duration(payload, 40);
        for (int i = 0; i < count; i++) {
            double angle = world.random.nextDouble() * Math.PI * 2.0;
            GameplayVfxUtil.spawnSprite(client, world, BongParticles.tribulationSparkSprites,
                origin[0], origin[1] + 0.2, origin[2],
                Math.cos(angle) * speed,
                0.05 + world.random.nextDouble() * speed,
                Math.sin(angle) * speed,
                rgb, 0.65f, age, 0.18f);
        }
    }

    private static void playFire(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload
    ) {
        playCloud(client, world, payload, 0xD0E0FF, 0.025, 0.08);
        playRing(client, world, payload, 0x801000, 0.02);
    }

    private static void playWither(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload
    ) {
        playBurst(client, world, payload, 0x806040, 0.04);
        playRing(client, world, payload, 0x383028, -0.02);
    }

    private static double radius(VfxEventPayload.SpawnParticle payload, double fallback) {
        return payload.direction()
            .map(direction -> Math.max(Math.abs(direction[0]), Math.abs(direction[2])))
            .orElse(fallback);
    }
}
