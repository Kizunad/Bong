package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class MovementVfxPlayer implements VfxPlayer {
    public static final Identifier DASH = new Identifier("bong", "movement_dash");
    public static final Identifier SLIDE = new Identifier("bong", "movement_slide");
    public static final Identifier DOUBLE_JUMP = new Identifier("bong", "movement_double_jump");

    private final Kind kind;

    public MovementVfxPlayer(Kind kind) {
        this.kind = kind;
    }

    public enum Kind {
        DASH,
        SLIDE,
        DOUBLE_JUMP
    }

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null || payload == null || payload.origin().length < 3) {
            return;
        }

        double[] origin = payload.origin();
        double ox = origin[0];
        double oy = origin[1];
        double oz = origin[2];
        float[] rgb = GameplayVfxUtil.rgb(payload, fallbackRgb());
        int count = GameplayVfxUtil.count(payload, defaultCount(), 1, 32);
        int maxAge = GameplayVfxUtil.duration(payload, defaultDuration());
        double[] dir = GameplayVfxUtil.direction(payload, new double[] { 0.0, 0.0, 1.0 });

        switch (kind) {
            case DASH -> playDash(client, world, ox, oy, oz, dir, rgb, count, maxAge);
            case SLIDE -> playSlide(client, world, ox, oy, oz, dir, rgb, count, maxAge);
            case DOUBLE_JUMP -> playDoubleJump(client, world, ox, oy, oz, rgb, count, maxAge);
        }
    }

    private static void playDash(
        MinecraftClient client,
        ClientWorld world,
        double ox,
        double oy,
        double oz,
        double[] dir,
        float[] rgb,
        int count,
        int maxAge
    ) {
        GameplayVfxUtil.spawnLine(client, world, BongParticles.flyingSwordTrailSprites,
            ox, oy + 0.15, oz, dir[0] * 0.35, 0.01, dir[2] * 0.35, rgb, 0.55f, maxAge, 0.045);
        for (int i = 0; i < count; i++) {
            GameplayVfxUtil.spawnSprite(client, world, BongParticles.cloudDustSprites,
                ox - dir[0] * world.random.nextDouble() * 0.8,
                oy + 0.05,
                oz - dir[2] * world.random.nextDouble() * 0.8,
                -dir[0] * (0.04 + world.random.nextDouble() * 0.06),
                0.02 + world.random.nextDouble() * 0.03,
                -dir[2] * (0.04 + world.random.nextDouble() * 0.06),
                rgb, 0.62f, maxAge, 0.11f);
        }
    }

    private static void playSlide(
        MinecraftClient client,
        ClientWorld world,
        double ox,
        double oy,
        double oz,
        double[] dir,
        float[] rgb,
        int count,
        int maxAge
    ) {
        for (int i = 0; i < count; i++) {
            GameplayVfxUtil.spawnSprite(client, world, BongParticles.cloudDustSprites,
                ox - dir[0] * world.random.nextDouble() * 0.7 + (world.random.nextDouble() - 0.5) * 0.25,
                oy + 0.02,
                oz - dir[2] * world.random.nextDouble() * 0.7 + (world.random.nextDouble() - 0.5) * 0.25,
                -dir[0] * 0.04 + (world.random.nextDouble() - 0.5) * 0.04,
                0.015,
                -dir[2] * 0.04 + (world.random.nextDouble() - 0.5) * 0.04,
                rgb, 0.7f, maxAge, 0.12f);
        }
        for (int i = 0; i < Math.max(1, count / 4); i++) {
            GameplayVfxUtil.spawnSprite(client, world, BongParticles.tribulationSparkSprites,
                ox + (world.random.nextDouble() - 0.5) * 0.5,
                oy + 0.05,
                oz + (world.random.nextDouble() - 0.5) * 0.5,
                (world.random.nextDouble() - 0.5) * 0.08,
                0.04,
                (world.random.nextDouble() - 0.5) * 0.08,
                new float[] { 1.0f, 0.53f, 0.0f }, 0.65f, Math.max(6, maxAge / 2), 0.07f);
        }
    }

    private static void playDoubleJump(
        MinecraftClient client,
        ClientWorld world,
        double ox,
        double oy,
        double oz,
        float[] rgb,
        int count,
        int maxAge
    ) {
        GameplayVfxUtil.spawnDecal(client, world, BongParticles.lingqiRippleSprites,
            ox, oy + 0.02, oz, rgb, 0.75f, maxAge, 0.55);
        for (int i = 0; i < count; i++) {
            double angle = (Math.PI * 2.0 * i) / Math.max(1, count);
            GameplayVfxUtil.spawnSprite(client, world, BongParticles.qiAuraSprites,
                ox + Math.cos(angle) * 0.25,
                oy + 0.08,
                oz + Math.sin(angle) * 0.25,
                Math.cos(angle) * 0.025,
                -0.08 - world.random.nextDouble() * 0.03,
                Math.sin(angle) * 0.025,
                rgb, 0.72f, maxAge, 0.10f);
        }
    }

    private int fallbackRgb() {
        return switch (kind) {
            case DASH -> 0xDDE6EE;
            case SLIDE -> 0x9B7653;
            case DOUBLE_JUMP -> 0xCCCCFF;
        };
    }

    private int defaultCount() {
        return switch (kind) {
            case DASH -> 10;
            case SLIDE -> 12;
            case DOUBLE_JUMP -> 8;
        };
    }

    private int defaultDuration() {
        return switch (kind) {
            case DASH -> 10;
            case SLIDE -> 12;
            case DOUBLE_JUMP -> 8;
        };
    }
}
