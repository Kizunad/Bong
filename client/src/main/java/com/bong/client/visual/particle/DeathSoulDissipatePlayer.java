package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.OptionalInt;

/**
 * {@code bong:death_soul_dissipate} —— 实体死亡的魂散（plan §4.4）。
 *
 * <p>origin 爆发式向各方向散出一批 qi_aura Sprite + 少量 rune_char Sprite，
 * 整体上飘，模拟"灵气飞散 / 魂归虚无"。
 */
public final class DeathSoulDissipatePlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "death_soul_dissipate");

    private static final int DEFAULT_AURA_COUNT = 20;
    private static final int DEFAULT_RUNE_COUNT = 3;
    private static final int FALLBACK_RGB = 0xCFEFFF;

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
        float alpha = (float) Math.max(0.4, Math.min(1.0, payload.strength().orElse(0.85)));

        int auraCount = clamp(payload.count().orElse(OptionalInt.of(DEFAULT_AURA_COUNT).getAsInt()), 1, 60);
        int maxAge = payload.durationTicks().orElse(OptionalInt.of(40).getAsInt());

        // aura: 球面爆发向各方向扩散，整体缓慢上飘
        for (int i = 0; i < auraCount; i++) {
            double theta = world.random.nextDouble() * Math.PI * 2;
            double phi = Math.acos(2 * world.random.nextDouble() - 1);
            double speed = 0.1 + world.random.nextDouble() * 0.15;
            double vx = Math.sin(phi) * Math.cos(theta) * speed;
            double vy = Math.cos(phi) * speed * 0.4 + 0.05; // 整体偏上飘
            double vz = Math.sin(phi) * Math.sin(theta) * speed;
            EnlightenmentAuraPlayer.spawnSprite(client, world, BongParticles.qiAuraSprites,
                ox, oy + 0.5, oz, vx, vy, vz, r, g, b, alpha, maxAge, 0.35f);
        }

        // 少量符文字符飘散（象征"遗言 / 魂归"）
        for (int i = 0; i < DEFAULT_RUNE_COUNT; i++) {
            double vx = (world.random.nextDouble() - 0.5) * 0.08;
            double vy = 0.03 + world.random.nextDouble() * 0.03;
            double vz = (world.random.nextDouble() - 0.5) * 0.08;
            EnlightenmentAuraPlayer.spawnSprite(client, world, BongParticles.runeCharSprites,
                ox + (world.random.nextDouble() - 0.5) * 0.4,
                oy + 0.8 + world.random.nextDouble() * 0.4,
                oz + (world.random.nextDouble() - 0.5) * 0.4,
                vx, vy, vz,
                1.0f, 0.9f, 0.55f,
                alpha, maxAge + 20, 0.3f);
        }
    }

    private static int clamp(int v, int lo, int hi) {
        return Math.max(lo, Math.min(hi, v));
    }
}
