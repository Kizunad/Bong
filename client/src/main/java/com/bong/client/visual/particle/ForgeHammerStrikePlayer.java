package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class ForgeHammerStrikePlayer implements VfxPlayer {
    public static final Identifier HAMMER = new Identifier("bong", "forge_hammer_strike");
    public static final Identifier INSCRIPTION = new Identifier("bong", "forge_inscription");
    public static final Identifier CONSECRATION = new Identifier("bong", "forge_consecration");

    private final Kind kind;

    public ForgeHammerStrikePlayer(Kind kind) {
        this.kind = kind;
    }

    public enum Kind {
        HAMMER,
        INSCRIPTION,
        CONSECRATION
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
        float[] rgb = GameplayVfxUtil.rgb(payload, fallbackRgb());
        int count = GameplayVfxUtil.count(payload, 8, 1, 32);
        int maxAge = GameplayVfxUtil.duration(payload, 20);

        if (kind == Kind.INSCRIPTION) {
            GameplayVfxUtil.spawnDecal(client, world, BongParticles.runeCharSprites,
                ox, oy, oz, rgb, 0.85f, maxAge, 0.7);
            return;
        }
        if (kind == Kind.CONSECRATION) {
            for (int i = 0; i < count; i++) {
                GameplayVfxUtil.spawnLine(client, world, BongParticles.breakthroughPillarSprites,
                    ox + (world.random.nextDouble() - 0.5) * 0.4,
                    oy,
                    oz + (world.random.nextDouble() - 0.5) * 0.4,
                    0.0, 0.9, 0.0, rgb, 0.8f, maxAge, 0.12);
            }
            return;
        }
        for (int i = 0; i < count; i++) {
            GameplayVfxUtil.spawnSprite(client, world, BongParticles.tribulationSparkSprites,
                ox, oy, oz,
                (world.random.nextDouble() - 0.5) * 0.18,
                0.05 + world.random.nextDouble() * 0.15,
                (world.random.nextDouble() - 0.5) * 0.18,
                rgb, 0.85f, maxAge, 0.09f);
        }
    }

    private int fallbackRgb() {
        return switch (kind) {
            case HAMMER -> 0xFF8800;
            case INSCRIPTION -> 0x4488FF;
            case CONSECRATION -> 0xFFFFFF;
        };
    }
}
