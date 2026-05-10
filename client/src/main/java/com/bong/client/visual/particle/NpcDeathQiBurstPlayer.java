package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.OptionalInt;

/**
 * Radial true-qi burst emitted when a high-realm NPC dies.
 */
public final class NpcDeathQiBurstPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "npc_death_qi_burst");

    private static final int FALLBACK_RGB = 0x8FE6B8;
    private static final int DEFAULT_COUNT = 8;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) return;

        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];

        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        double strength = Math.max(0.25, Math.min(1.25, payload.strength().orElse(0.75)));
        int count = clamp(payload.count().orElse(OptionalInt.of(DEFAULT_COUNT).getAsInt()), 4, 24);
        int maxAge = payload.durationTicks().orElse(OptionalInt.of(12).getAsInt());

        for (int i = 0; i < count; i++) {
            double angle = (Math.PI * 2.0 * i / count) + world.random.nextDouble() * 0.08;
            double speed = 0.38 + strength * 0.34;
            double vx = Math.cos(angle) * speed;
            double vz = Math.sin(angle) * speed;
            double vy = 0.05 + world.random.nextDouble() * 0.08;
            BongLineParticle particle = new BongLineParticle(
                world,
                ox,
                oy + world.random.nextDouble() * 0.25,
                oz,
                vx,
                vy,
                vz
            );
            particle.setLineShape(1.2, 0.55, 0.08 + 0.04 * strength);
            particle.setColor(r, g, b);
            particle.setAlphaPublic((float) Math.max(0.35, Math.min(0.9, strength)));
            particle.setMaxAgePublic(maxAge);
            if (BongParticles.qiAuraSprites != null) {
                particle.setSpritePublic(BongParticles.qiAuraSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    private static int clamp(int v, int lo, int hi) {
        return Math.max(lo, Math.min(hi, v));
    }
}
