package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.OptionalInt;

/**
 * {@code bong:formation_activate} —— 符阵激活（plan §4.4）。
 *
 * <p>origin 贴地画出 lingqi_ripple GroundDecal（同心圆），自转 + 短时间存在。
 * strength 决定缩放（更强的阵 → 更大的涟漪）。
 */
public final class FormationActivatePlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "formation_activate");

    private static final int FALLBACK_RGB = 0xC4E0FF;
    private static final double BASE_HALF_SIZE = 1.5;

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
        double strength = payload.strength().orElse(0.9);
        float alpha = (float) Math.max(0.3, Math.min(1.0, strength));

        int maxAge = payload.durationTicks().orElse(OptionalInt.of(80).getAsInt());
        double halfSize = BASE_HALF_SIZE * (0.6 + 0.8 * strength);

        BongGroundDecalParticle p = new BongGroundDecalParticle(world, ox, oy, oz);
        p.setDecalShape(halfSize, 0.02);
        p.setSpin(world.random.nextDouble() * Math.PI * 2, 0.04);  // 缓慢自转 0.04 rad/tick
        p.setColor(r, g, b);
        p.setAlphaPublic(alpha);
        p.setMaxAgePublic(maxAge);
        if (BongParticles.lingqiRippleSprites != null) {
            p.setSpritePublic(BongParticles.lingqiRippleSprites.getSprite(world.random));
        }
        client.particleManager.addParticle(p);
    }
}
