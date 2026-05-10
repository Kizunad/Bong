package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class LingtianActionVfxPlayer implements VfxPlayer {
    public static final Identifier TILL = new Identifier("bong", "lingtian_till");
    public static final Identifier PLANT = new Identifier("bong", "lingtian_plant");
    public static final Identifier REPLENISH = new Identifier("bong", "lingtian_replenish");

    private final Kind kind;

    public LingtianActionVfxPlayer(Kind kind) {
        this.kind = kind;
    }

    public enum Kind {
        TILL,
        PLANT,
        REPLENISH
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
        int count = GameplayVfxUtil.count(payload, kind == Kind.PLANT ? 4 : 8, 1, 24);
        int maxAge = GameplayVfxUtil.duration(payload, 30);

        if (kind == Kind.PLANT) {
            GameplayVfxUtil.spawnDecal(client, world, BongParticles.lingqiRippleSprites,
                ox, oy, oz, rgb, 0.75f, maxAge, 0.6);
            return;
        }
        for (int i = 0; i < count; i++) {
            double vy = kind == Kind.REPLENISH ? -0.05 : 0.08 + world.random.nextDouble() * 0.08;
            double py = kind == Kind.REPLENISH ? oy + 1.5 + world.random.nextDouble() * 0.5 : oy;
            GameplayVfxUtil.spawnSprite(client, world,
                kind == Kind.REPLENISH ? BongParticles.qiAuraSprites : BongParticles.qiAuraSprites,
                ox + (world.random.nextDouble() - 0.5) * 0.8,
                py,
                oz + (world.random.nextDouble() - 0.5) * 0.8,
                (world.random.nextDouble() - 0.5) * 0.06,
                vy,
                (world.random.nextDouble() - 0.5) * 0.06,
                rgb, 0.65f, maxAge, 0.12f);
        }
    }

    private int fallbackRgb() {
        return switch (kind) {
            case TILL -> 0x8B5A2B;
            case PLANT -> 0x44AA44;
            case REPLENISH -> 0x66FFCC;
        };
    }
}
