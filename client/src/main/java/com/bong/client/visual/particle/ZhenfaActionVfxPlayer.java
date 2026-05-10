package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class ZhenfaActionVfxPlayer implements VfxPlayer {
    public static final Identifier TRAP = new Identifier("bong", "zhenfa_trap");
    public static final Identifier WARD = new Identifier("bong", "zhenfa_ward");
    public static final Identifier DEPLETE = new Identifier("bong", "zhenfa_deplete");

    private final Kind kind;

    public ZhenfaActionVfxPlayer(Kind kind) {
        this.kind = kind;
    }

    public enum Kind {
        TRAP,
        WARD,
        DEPLETE
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
        int maxAge = GameplayVfxUtil.duration(payload, kind == Kind.WARD ? 60 : 30);
        double halfSize = kind == Kind.WARD ? 1.8 : 1.1;
        GameplayVfxUtil.spawnDecal(client, world, BongParticles.lingqiRippleSprites,
            ox, oy, oz, rgb, 0.75f, maxAge, halfSize);
        int count = GameplayVfxUtil.count(payload, kind == Kind.WARD ? 20 : 12, 1, 48);
        for (int i = 0; i < count; i++) {
            double theta = world.random.nextDouble() * Math.PI * 2.0;
            double speed = kind == Kind.DEPLETE ? 0.03 : 0.10;
            GameplayVfxUtil.spawnSprite(client, world, BongParticles.runeCharSprites,
                ox, oy + 0.2, oz,
                Math.cos(theta) * speed,
                0.02 + world.random.nextDouble() * 0.08,
                Math.sin(theta) * speed,
                rgb, 0.65f, maxAge, 0.18f);
        }
    }

    private int fallbackRgb() {
        return switch (kind) {
            case TRAP -> 0xFF3344;
            case WARD -> 0x4488FF;
            case DEPLETE -> 0x888888;
        };
    }
}
