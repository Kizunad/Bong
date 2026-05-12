package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.Objects;

public final class MovementVfxPlayer implements VfxPlayer {
    public static final Identifier DASH = new Identifier("bong", "movement_dash");

    private final Kind kind;

    public MovementVfxPlayer(Kind kind) {
        this.kind = Objects.requireNonNull(kind, "kind");
    }

    public enum Kind {
        DASH
    }

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null || payload == null) {
            return;
        }

        double[] origin = payload.origin();
        if (origin == null || origin.length < 3) {
            return;
        }
        double ox = origin[0];
        double oy = origin[1];
        double oz = origin[2];
        float[] rgb = GameplayVfxUtil.rgb(payload, fallbackRgb());
        int count = GameplayVfxUtil.count(payload, defaultCount(), 1, 32);
        int maxAge = GameplayVfxUtil.duration(payload, defaultDuration());
        double[] dir = GameplayVfxUtil.direction(payload, new double[] { 0.0, 0.0, 1.0 });

        switch (kind) {
            case DASH -> playDash(client, world, ox, oy, oz, dir, rgb, count, maxAge);
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

    private int fallbackRgb() {
        return switch (kind) {
            case DASH -> 0xDDE6EE;
        };
    }

    private int defaultCount() {
        return switch (kind) {
            case DASH -> 10;
        };
    }

    private int defaultDuration() {
        return switch (kind) {
            case DASH -> 10;
        };
    }
}
