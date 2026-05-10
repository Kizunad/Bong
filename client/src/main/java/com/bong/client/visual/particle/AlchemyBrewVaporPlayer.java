package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class AlchemyBrewVaporPlayer implements VfxPlayer {
    public static final Identifier BREW = new Identifier("bong", "alchemy_brew_vapor");
    public static final Identifier OVERHEAT = new Identifier("bong", "alchemy_overheat");
    public static final Identifier COMPLETE = new Identifier("bong", "alchemy_complete");
    public static final Identifier EXPLODE = new Identifier("bong", "alchemy_explode");

    private final Kind kind;

    public AlchemyBrewVaporPlayer(Kind kind) {
        this.kind = kind;
    }

    public enum Kind {
        BREW,
        OVERHEAT,
        COMPLETE,
        EXPLODE
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
        int count = GameplayVfxUtil.count(payload, defaultCount(), 1, 48);
        int maxAge = GameplayVfxUtil.duration(payload, kind == Kind.COMPLETE ? 40 : 30);

        if (kind == Kind.COMPLETE) {
            GameplayVfxUtil.spawnSprite(client, world, BongParticles.enlightenmentDustSprites,
                ox, oy, oz, 0.0, 0.05, 0.0, rgb, 0.9f, maxAge, 0.25f);
            return;
        }
        for (int i = 0; i < count; i++) {
            double spread = kind == Kind.EXPLODE ? 0.18 : 0.08;
            GameplayVfxUtil.spawnSprite(client, world, particleProvider(),
                ox + (world.random.nextDouble() - 0.5) * 0.5,
                oy,
                oz + (world.random.nextDouble() - 0.5) * 0.5,
                (world.random.nextDouble() - 0.5) * spread,
                0.03 + world.random.nextDouble() * (kind == Kind.EXPLODE ? 0.12 : 0.05),
                (world.random.nextDouble() - 0.5) * spread,
                rgb, 0.65f, maxAge, kind == Kind.BREW ? 0.16f : 0.11f);
        }
    }

    private int fallbackRgb() {
        return switch (kind) {
            case BREW -> 0x88CCFF;
            case OVERHEAT, EXPLODE -> 0xFF5533;
            case COMPLETE -> 0xFFD700;
        };
    }

    private int defaultCount() {
        return switch (kind) {
            case BREW -> 8;
            case OVERHEAT -> 10;
            case COMPLETE -> 1;
            case EXPLODE -> 18;
        };
    }

    private net.minecraft.client.particle.SpriteProvider particleProvider() {
        return kind == Kind.BREW ? BongParticles.qiAuraSprites : BongParticles.tribulationSparkSprites;
    }
}
