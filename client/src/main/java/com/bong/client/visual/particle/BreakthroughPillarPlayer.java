package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import com.bong.client.season.SeasonBreakthroughOverlay;
import com.bong.client.season.SeasonBreakthroughOverlayHud;
import com.bong.client.state.SeasonStateStore;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.particle.SpriteProvider;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.OptionalInt;

/**
 * {@code bong:breakthrough_pillar} —— 境界突破的垂直破境光柱（plan §4.4）。
 *
 * <p>视觉：origin 地面升起 N 条 Line 粒子，velocity 纯 +Y 向上；微量水平抖动给体积感；
 * maxAge 较长（默认 60 tick，3s）以撑住破境仪式感。
 */
public final class BreakthroughPillarPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "breakthrough_pillar");

    private static final int DEFAULT_COUNT = 12;
    private static final int FALLBACK_RGB = 0xFFE8A0; // 金白
    private static final double SPEED = 1.4;
    private static final double SCATTER = 0.25;
    private float intensityMultiplier = 1.0f;
    private Integer overrideColorRgb;
    private float heightMultiplier = 1.0f;

    public void setIntensity(float intensityMultiplier) {
        this.intensityMultiplier = Math.max(0.1f, Math.min(2.0f, intensityMultiplier));
    }

    public void setColor(int rgb) {
        this.overrideColorRgb = rgb & 0xFFFFFF;
    }

    public void setHeight(float heightMultiplier) {
        this.heightMultiplier = Math.max(0.25f, Math.min(4.0f, heightMultiplier));
    }

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) return;

        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];

        SeasonBreakthroughOverlay.BreakthroughProfile seasonProfile =
            SeasonBreakthroughOverlay.breakthroughProfile(SeasonStateStore.snapshot(), true, world.getTime());
        SeasonBreakthroughOverlayHud.trigger(seasonProfile, System.currentTimeMillis());
        int rgb = payload.colorRgb().orElse(
            overrideColorRgb != null ? overrideColorRgb : seasonProfile.pillarTintRgb() == 0 ? FALLBACK_RGB : seasonProfile.pillarTintRgb()
        );
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        float alpha = (float) Math.max(0.4, Math.min(1.0, payload.strength().orElse(1.0) * intensityMultiplier));

        int count = clamp(
            (int) Math.round(payload.count().orElse(OptionalInt.of(DEFAULT_COUNT).getAsInt()) * seasonProfile.lightningMultiplier()),
            1,
            64
        );
        int maxAge = payload.durationTicks().orElse(OptionalInt.of(60).getAsInt());

        for (int i = 0; i < count; i++) {
            double jitterX = (world.random.nextDouble() - 0.5) * 2 * SCATTER;
            double jitterZ = (world.random.nextDouble() - 0.5) * 2 * SCATTER;
            double yOffset = world.random.nextDouble() * 0.4;

            BongLineParticle p = new BongLineParticle(
                world,
                ox + jitterX, oy + yOffset, oz + jitterZ,
                0.0, (SPEED + world.random.nextDouble() * 0.4) * heightMultiplier, 0.0
            );
            p.setLineShape(0.8, 1.5 * heightMultiplier, 0.18);
            p.setColor(r, g, b);
            p.setAlphaPublic(alpha);
            p.setMaxAgePublic(maxAge);
            SpriteProvider spriteProvider = spriteProviderFor(seasonProfile);
            if (spriteProvider != null) {
                p.setSpritePublic(spriteProvider.getSprite(world.random));
            }
            client.particleManager.addParticle(p);
        }
    }

    private static SpriteProvider spriteProviderFor(SeasonBreakthroughOverlay.BreakthroughProfile profile) {
        return switch (profile.particleSpriteId()) {
            case "enlightenment_dust" -> BongParticles.enlightenmentDustSprites;
            case "tribulation_spark" -> BongParticles.tribulationSparkSprites;
            default -> BongParticles.breakthroughPillarSprites;
        };
    }

    private static int clamp(int v, int lo, int hi) {
        return Math.max(lo, Math.min(hi, v));
    }
}
