package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class SwordBasicsVfxPlayer implements VfxPlayer {
    public static final Identifier CLEAVE_TRAIL = new Identifier("bong", "sword_cleave_trail");
    public static final Identifier THRUST_HIT = new Identifier("bong", "sword_thrust_hit");
    public static final Identifier PARRY_SPARK = new Identifier("bong", "sword_parry_spark");
    public static final Identifier INFUSE_GLOW = new Identifier("bong", "sword_infuse_glow");

    public enum Kind {
        CLEAVE,
        THRUST,
        PARRY,
        INFUSE
    }

    private final Kind kind;

    public SwordBasicsVfxPlayer(Kind kind) {
        this.kind = java.util.Objects.requireNonNull(kind, "kind");
    }

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }
        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];
        float[] rgb = GameplayVfxUtil.rgb(payload, fallbackRgb(kind));
        int duration = GameplayVfxUtil.duration(payload, fallbackDuration(kind));
        int count = GameplayVfxUtil.count(payload, fallbackCount(kind), 1, 24);

        switch (kind) {
            case CLEAVE -> playCleave(client, world, payload, ox, oy, oz, rgb, duration);
            case THRUST -> playThrust(client, world, ox, oy, oz, rgb, duration, count);
            case PARRY -> playParry(client, world, ox, oy, oz, rgb, duration, count);
            case INFUSE -> playInfuse(client, world, ox, oy, oz, rgb, duration, count);
        }
    }

    private static void playCleave(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        double ox,
        double oy,
        double oz,
        float[] rgb,
        int duration
    ) {
        double[] dir = GameplayVfxUtil.direction(payload, new double[] { 0.7, -0.4, 0.0 });
        GameplayVfxUtil.spawnLine(
            client,
            world,
            BongParticles.swordSlashArcSprites,
            ox,
            oy,
            oz,
            dir[0] * 0.9,
            dir[1] * 0.9,
            dir[2] * 0.9,
            rgb,
            0.85f,
            duration,
            0.14
        );
    }

    private static void playThrust(
        MinecraftClient client,
        ClientWorld world,
        double ox,
        double oy,
        double oz,
        float[] rgb,
        int duration,
        int count
    ) {
        for (int i = 0; i < count; i++) {
            GameplayVfxUtil.spawnSprite(
                client,
                world,
                BongParticles.tribulationSparkSprites,
                ox,
                oy,
                oz,
                (world.random.nextDouble() - 0.5) * 0.06,
                0.01 + world.random.nextDouble() * 0.04,
                (world.random.nextDouble() - 0.5) * 0.06,
                rgb,
                0.75f,
                duration,
                0.08f
            );
        }
    }

    private static void playParry(
        MinecraftClient client,
        ClientWorld world,
        double ox,
        double oy,
        double oz,
        float[] rgb,
        int duration,
        int count
    ) {
        for (int i = 0; i < count; i++) {
            GameplayVfxUtil.spawnSprite(
                client,
                world,
                BongParticles.tribulationSparkSprites,
                ox,
                oy,
                oz,
                (world.random.nextDouble() - 0.5) * 0.18,
                0.04 + world.random.nextDouble() * 0.14,
                (world.random.nextDouble() - 0.5) * 0.18,
                rgb,
                0.9f,
                duration,
                0.10f
            );
        }
    }

    private static void playInfuse(
        MinecraftClient client,
        ClientWorld world,
        double ox,
        double oy,
        double oz,
        float[] rgb,
        int duration,
        int count
    ) {
        for (int i = 0; i < count; i++) {
            GameplayVfxUtil.spawnSprite(
                client,
                world,
                BongParticles.swordQiTrailSprites,
                ox + (world.random.nextDouble() - 0.5) * 0.18,
                oy + world.random.nextDouble() * 0.8,
                oz + (world.random.nextDouble() - 0.5) * 0.18,
                0.0,
                0.015,
                0.0,
                rgb,
                0.55f,
                duration,
                0.09f
            );
        }
    }

    private static int fallbackRgb(Kind kind) {
        return switch (kind) {
            case CLEAVE -> 0xC0C0C8;
            case THRUST -> 0xC03030;
            case PARRY -> 0xFFD080;
            case INFUSE -> 0xB0E0C0;
        };
    }

    private static int fallbackDuration(Kind kind) {
        return switch (kind) {
            case CLEAVE -> 4;
            case THRUST -> 4;
            case PARRY -> 3;
            case INFUSE -> 20;
        };
    }

    private static int fallbackCount(Kind kind) {
        return switch (kind) {
            case CLEAVE -> 1;
            case THRUST -> 2;
            case PARRY -> 4;
            case INFUSE -> 1;
        };
    }
}
