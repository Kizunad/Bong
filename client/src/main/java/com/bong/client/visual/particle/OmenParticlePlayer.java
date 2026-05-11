package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import com.bong.client.omen.OmenStateStore;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class OmenParticlePlayer implements VfxPlayer {
    public static final Identifier PSEUDO_VEIN =
        new Identifier("bong", "world_omen_pseudo_vein");
    public static final Identifier BEAST_TIDE =
        new Identifier("bong", "world_omen_beast_tide");
    public static final Identifier REALM_COLLAPSE =
        new Identifier("bong", "world_omen_realm_collapse");
    public static final Identifier KARMA_BACKLASH =
        new Identifier("bong", "world_omen_karma_backlash");

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        OmenStateStore.note(payload, System.currentTimeMillis());
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }

        double[] origin = payload.origin();
        float[] rgb = GameplayVfxUtil.rgb(payload, fallbackColor(payload.eventId()));
        int count = GameplayVfxUtil.count(payload, 18, 4, 48);
        int maxAge = GameplayVfxUtil.duration(payload, 120);
        double strength = GameplayVfxUtil.strength(payload, 0.6);

        for (int i = 0; i < count; i++) {
            double angle = world.random.nextDouble() * Math.PI * 2.0;
            double radius = 2.0 + world.random.nextDouble() * (3.0 + strength * 5.0);
            double x = origin[0] + Math.cos(angle) * radius;
            double y = origin[1] + 0.5 + world.random.nextDouble() * 3.0;
            double z = origin[2] + Math.sin(angle) * radius;
            double drift = 0.01 + strength * 0.025;
            GameplayVfxUtil.spawnSprite(
                client,
                world,
                spriteProvider(payload.eventId()),
                x,
                y,
                z,
                Math.cos(angle) * drift,
                0.006 + world.random.nextDouble() * 0.015,
                Math.sin(angle) * drift,
                rgb,
                (float) (0.28 + strength * 0.35),
                maxAge,
                (float) (0.12 + strength * 0.18)
            );
        }
    }

    private static int fallbackColor(Identifier eventId) {
        if (BEAST_TIDE.equals(eventId)) return 0xB8864A;
        if (REALM_COLLAPSE.equals(eventId)) return 0x7A1E24;
        if (KARMA_BACKLASH.equals(eventId)) return 0xA01830;
        return 0x66D8C8;
    }

    private static net.minecraft.client.particle.SpriteProvider spriteProvider(Identifier eventId) {
        if (REALM_COLLAPSE.equals(eventId) || KARMA_BACKLASH.equals(eventId)) {
            return BongParticles.tribulationSparkSprites;
        }
        return BongParticles.qiAuraSprites;
    }
}
