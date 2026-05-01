package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

/**
 * {@code bong:tribulation_boundary} —— 渡虚劫锁定圈地面投影。
 *
 * <p>服务端用 strength 编码半径比例：1.0=100 格预兆圈，0.2=20 格锁定圈，
 * 0.1=首波后的 10 格硬圈。只做视觉提示，实际越界判定仍在服务端。
 */
public final class TribulationBoundaryPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "tribulation_boundary");

    private static final int FALLBACK_RGB = 0xD0C8FF;
    private static final double MAX_RADIUS = 100.0;
    private static final double MIN_RADIUS = 5.0;
    private static final int DEFAULT_DURATION_TICKS = 100;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) return;

        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];
        double strength = clamp(payload.strength().orElse(0.2), 0.0, 1.0);
        double radius = Math.max(MIN_RADIUS, strength * MAX_RADIUS);
        int maxAge = payload.durationTicks().orElse(DEFAULT_DURATION_TICKS);

        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        float alpha = radius <= 10.5 ? 0.82f : radius <= 20.5 ? 0.68f : 0.46f;

        BongGroundDecalParticle p = new BongGroundDecalParticle(world, ox, oy, oz);
        p.setDecalShape(radius, 0.04);
        p.setSpin(world.random.nextDouble() * Math.PI * 2, -0.012);
        p.setColor(r, g, b);
        p.setAlphaPublic(alpha);
        p.setMaxAgePublic(maxAge);
        if (BongParticles.lingqiRippleSprites != null) {
            p.setSpritePublic(BongParticles.lingqiRippleSprites.getSprite(world.random));
        }
        client.particleManager.addParticle(p);
    }

    private static double clamp(double value, double min, double max) {
        return Math.max(min, Math.min(max, value));
    }
}
