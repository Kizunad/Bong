package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.OptionalInt;

/**
 * {@code bong:enlightenment_aura} —— 顿悟事件的散布星屑（plan §4.4）。
 *
 * <p>大量 enlightenment_dust Sprite 粒子环绕 origin 缓慢飘浮，
 * 混少量 rune_char Sprite 飘字增加"领悟天道"感。
 */
public final class EnlightenmentAuraPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "enlightenment_aura");

    private static final int DEFAULT_DUST_COUNT = 24;
    private static final int DEFAULT_RUNE_COUNT = 4;
    private static final int FALLBACK_RGB = 0xFFE8B0;

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
        float alpha = (float) Math.max(0.4, Math.min(1.0, payload.strength().orElse(0.9)));

        int dustCount = clamp(payload.count().orElse(OptionalInt.of(DEFAULT_DUST_COUNT).getAsInt()), 1, 64);
        int maxAge = payload.durationTicks().orElse(OptionalInt.of(50).getAsInt());

        // 散布 dust
        for (int i = 0; i < dustCount; i++) {
            double dx = (world.random.nextDouble() - 0.5) * 2.0;
            double dy = world.random.nextDouble() * 1.8;
            double dz = (world.random.nextDouble() - 0.5) * 2.0;
            double vx = (world.random.nextDouble() - 0.5) * 0.05;
            double vy = 0.02 + world.random.nextDouble() * 0.04;
            double vz = (world.random.nextDouble() - 0.5) * 0.05;
            spawnSprite(client, world, BongParticles.enlightenmentDustSprites,
                ox + dx, oy + dy, oz + dz, vx, vy, vz, r, g, b, alpha, maxAge, 0.08f);
        }

        // 少量 rune_char 缓慢飘
        for (int i = 0; i < DEFAULT_RUNE_COUNT; i++) {
            double dx = (world.random.nextDouble() - 0.5) * 1.6;
            double dy = 1.2 + world.random.nextDouble() * 0.6;
            double dz = (world.random.nextDouble() - 0.5) * 1.6;
            double vy = 0.01 + world.random.nextDouble() * 0.02;
            spawnSprite(client, world, BongParticles.runeCharSprites,
                ox + dx, oy + dy, oz + dz, 0, vy, 0, 1.0f, 0.85f, 0.35f, alpha, maxAge + 20, 0.3f);
        }
    }

    static void spawnSprite(
        MinecraftClient client, ClientWorld world,
        net.minecraft.client.particle.SpriteProvider provider,
        double x, double y, double z, double vx, double vy, double vz,
        float r, float g, float b, float alpha, int maxAge, float scale
    ) {
        if (provider == null) return;
        BongSpriteParticle p = new BongSpriteParticle(world, x, y, z, vx, vy, vz);
        p.setSpritePublic(provider.getSprite(world.random));
        p.setColor(r, g, b);
        p.setAlphaPublic(alpha);
        p.setScalePublic(scale);
        p.setMaxAgePublic(maxAge);
        client.particleManager.addParticle(p);
    }

    private static int clamp(int v, int lo, int hi) {
        return Math.max(lo, Math.min(hi, v));
    }
}
