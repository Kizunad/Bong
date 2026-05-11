package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.particle.ParticleTypes;
import net.minecraft.util.Identifier;

public final class PseudoVeinVisualPlayer implements VfxPlayer {
    public static final Identifier RISING = new Identifier("bong", "pseudo_vein_rising");
    public static final Identifier ACTIVE = new Identifier("bong", "pseudo_vein_active");
    public static final Identifier WARNING = new Identifier("bong", "pseudo_vein_warning");
    public static final Identifier DISSIPATING = new Identifier("bong", "pseudo_vein_dissipating");
    public static final Identifier AFTERMATH = new Identifier("bong", "pseudo_vein_aftermath");

    private static final int GOLD = 0xFFD36A;
    private static final int WARNING_GOLD = 0xCFA84A;
    private static final int ASH = 0x8C8C82;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }

        Identifier eventId = payload.eventId();
        if (eventId.equals(AFTERMATH)) {
            spawnAftermath(world, payload);
            return;
        }
        if (eventId.equals(DISSIPATING)) {
            spawnDissipating(client, world, payload);
            return;
        }

        spawnPillar(client, world, payload, eventId.equals(WARNING));
        if (eventId.equals(WARNING)) {
            spawnWarningAsh(world, payload);
        } else if (eventId.equals(ACTIVE)) {
            spawnGroundGlow(client, world, payload);
        }
    }

    private static void spawnPillar(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        boolean warning
    ) {
        double[] origin = payload.origin();
        float[] rgb = GameplayVfxUtil.rgb(payload, warning ? WARNING_GOLD : GOLD);
        int count = GameplayVfxUtil.count(payload, warning ? 28 : 20, 4, 64);
        int maxAge = GameplayVfxUtil.duration(payload, warning ? 70 : 110);
        float alpha = (float) Math.max(0.35, GameplayVfxUtil.strength(payload, warning ? 0.75 : 0.65));

        for (int i = 0; i < count; i++) {
            double ring = warning ? 1.2 : 0.75;
            double angle = world.random.nextDouble() * Math.PI * 2.0;
            double radius = world.random.nextDouble() * ring;
            double x = origin[0] + Math.cos(angle) * radius;
            double z = origin[2] + Math.sin(angle) * radius;
            double y = origin[1] + world.random.nextDouble() * 0.35;
            GameplayVfxUtil.spawnLine(
                client,
                world,
                BongParticles.breakthroughPillarSprites,
                x,
                y,
                z,
                0.0,
                warning ? 1.1 : 0.8 + world.random.nextDouble() * 0.35,
                0.0,
                rgb,
                alpha,
                maxAge,
                warning ? 0.16 : 0.11
            );
        }
    }

    private static void spawnGroundGlow(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload
    ) {
        double[] origin = payload.origin();
        float[] rgb = GameplayVfxUtil.rgb(payload, 0x70D66D);
        int count = GameplayVfxUtil.count(payload, 12, 4, 24);
        for (int i = 0; i < count; i++) {
            double x = origin[0] + (world.random.nextDouble() - 0.5) * 8.0;
            double z = origin[2] + (world.random.nextDouble() - 0.5) * 8.0;
            GameplayVfxUtil.spawnDecal(
                client,
                world,
                BongParticles.enlightenmentDustSprites,
                x,
                origin[1] + 0.05,
                z,
                rgb,
                0.35f,
                80,
                0.45 + world.random.nextDouble() * 0.35
            );
        }
    }

    private static void spawnWarningAsh(ClientWorld world, VfxEventPayload.SpawnParticle payload) {
        double[] origin = payload.origin();
        int count = GameplayVfxUtil.count(payload, 18, 4, 40);
        for (int i = 0; i < count; i++) {
            double x = origin[0] + (world.random.nextDouble() - 0.5) * 6.0;
            double z = origin[2] + (world.random.nextDouble() - 0.5) * 6.0;
            world.addParticle(
                ParticleTypes.SMOKE,
                x,
                origin[1] + world.random.nextDouble() * 1.2,
                z,
                0.0,
                0.02,
                0.0
            );
        }
    }

    private static void spawnDissipating(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload
    ) {
        double[] origin = payload.origin();
        float[] rgb = GameplayVfxUtil.rgb(payload, ASH);
        int count = GameplayVfxUtil.count(payload, 22, 4, 48);
        for (int i = 0; i < count; i++) {
            double angle = world.random.nextDouble() * Math.PI * 2.0;
            double radius = 0.5 + world.random.nextDouble() * 4.5;
            double x = origin[0] + Math.cos(angle) * radius;
            double z = origin[2] + Math.sin(angle) * radius;
            GameplayVfxUtil.spawnDecal(
                client,
                world,
                BongParticles.enlightenmentDustSprites,
                x,
                origin[1] + 0.04,
                z,
                rgb,
                0.40f,
                90,
                0.55
            );
            world.addParticle(ParticleTypes.CLOUD, x, origin[1] + 0.3, z, 0.0, 0.01, 0.0);
        }
    }

    private static void spawnAftermath(ClientWorld world, VfxEventPayload.SpawnParticle payload) {
        double[] origin = payload.origin();
        int count = GameplayVfxUtil.count(payload, 30, 4, 64);
        double strength = GameplayVfxUtil.strength(payload, 0.65);
        for (int i = 0; i < count; i++) {
            double angle = world.random.nextDouble() * Math.PI * 2.0;
            double radius = 2.0 + world.random.nextDouble() * 10.0;
            double x = origin[0] + Math.cos(angle) * radius;
            double z = origin[2] + Math.sin(angle) * radius;
            double inward = -0.02 * strength;
            world.addParticle(
                ParticleTypes.WITCH,
                x,
                origin[1] + 0.2 + world.random.nextDouble() * 1.4,
                z,
                Math.cos(angle) * inward,
                0.01,
                Math.sin(angle) * inward
            );
        }
    }
}
