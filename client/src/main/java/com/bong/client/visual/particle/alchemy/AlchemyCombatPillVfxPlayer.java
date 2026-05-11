package com.bong.client.visual.particle.alchemy;

import com.bong.client.network.VfxEventPayload;
import com.bong.client.visual.particle.BongLineParticle;
import com.bong.client.visual.particle.BongParticles;
import com.bong.client.visual.particle.BongRibbonParticle;
import com.bong.client.visual.particle.BongSpriteParticle;
import com.bong.client.visual.particle.VfxPlayer;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.particle.SpriteProvider;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.List;

public final class AlchemyCombatPillVfxPlayer implements VfxPlayer {
    public static final Identifier HUO_XUE = id("pill_huo_xue");
    public static final Identifier XU_GU = id("pill_xu_gu");
    public static final Identifier DUAN_XU = id("pill_duan_xu");
    public static final Identifier TIE_BI = id("pill_tie_bi");
    public static final Identifier JIN_ZHONG = id("pill_jin_zhong");
    public static final Identifier NING_JIA = id("pill_ning_jia");
    public static final Identifier JI_FENG = id("pill_ji_feng");
    public static final Identifier SUO_DI = id("pill_suo_di");
    public static final Identifier HUI_LI = id("pill_hui_li");
    public static final Identifier HU_GU = id("pill_hu_gu");

    public static final List<Identifier> EVENT_IDS = List.of(
        HUO_XUE,
        XU_GU,
        DUAN_XU,
        TIE_BI,
        JIN_ZHONG,
        NING_JIA,
        JI_FENG,
        SUO_DI,
        HUI_LI,
        HU_GU
    );

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client == null ? null : client.world;
        if (world == null || client.particleManager == null || payload == null) {
            return;
        }
        double[] origin = payload.origin();
        if (origin == null || origin.length != 3) {
            return;
        }
        Profile profile = Profile.forEvent(payload.eventId());
        if (profile == null) {
            return;
        }

        int count = clamp(payload.count().orElse(profile.count), 1, 48);
        int maxAge = clamp(payload.durationTicks().orElse(profile.durationTicks), 4, 120);
        double strength = payload.strength().orElse(0.75).doubleValue();
        float[] rgb = rgb(payload.colorRgb().orElse(profile.rgb));
        switch (profile.shape) {
            case SPRITE -> spawnSpriteCloud(client, world, profile.provider(), origin, rgb, count, maxAge, strength);
            case LINE -> spawnLineBurst(client, world, profile.provider(), origin, rgb, count, maxAge, strength);
            case RIBBON -> spawnRibbons(client, world, profile.provider(), origin, rgb, count, maxAge, strength);
        }
    }

    private static void spawnSpriteCloud(
        MinecraftClient client,
        ClientWorld world,
        SpriteProvider provider,
        double[] origin,
        float[] rgb,
        int count,
        int maxAge,
        double strength
    ) {
        if (provider == null) {
            return;
        }
        for (int i = 0; i < count; i++) {
            double angle = Math.PI * 2.0 * i / Math.max(1, count);
            double radius = 0.18 + world.random.nextDouble() * (0.45 + strength * 0.55);
            BongSpriteParticle particle = new BongSpriteParticle(
                world,
                origin[0] + Math.cos(angle) * radius * 0.35,
                origin[1] + 0.35 + world.random.nextDouble() * 0.75,
                origin[2] + Math.sin(angle) * radius * 0.35,
                Math.cos(angle) * 0.018 * strength,
                0.012 + world.random.nextDouble() * 0.025,
                Math.sin(angle) * 0.018 * strength
            );
            particle.setSpritePublic(provider.getSprite(world.random));
            particle.setColor(rgb[0], rgb[1], rgb[2]);
            particle.setAlphaPublic(0.50f + (float) strength * 0.28f);
            particle.setMaxAgePublic(maxAge + world.random.nextInt(Math.max(1, maxAge / 3)));
            particle.setScalePublic(0.09f + (float) strength * 0.08f);
            client.particleManager.addParticle(particle);
        }
    }

    private static void spawnLineBurst(
        MinecraftClient client,
        ClientWorld world,
        SpriteProvider provider,
        double[] origin,
        float[] rgb,
        int count,
        int maxAge,
        double strength
    ) {
        if (provider == null) {
            return;
        }
        for (int i = 0; i < count; i++) {
            double angle = Math.PI * 2.0 * i / Math.max(1, count);
            double lift = 0.02 + world.random.nextDouble() * 0.08;
            BongLineParticle particle = new BongLineParticle(
                world,
                origin[0],
                origin[1] + 0.15 + world.random.nextDouble() * 0.45,
                origin[2],
                Math.cos(angle) * (0.12 + 0.12 * strength),
                lift,
                Math.sin(angle) * (0.12 + 0.12 * strength)
            );
            particle.setLineShape(1.7, 0.45, 0.035 + strength * 0.025);
            particle.setSpritePublic(provider.getSprite(world.random));
            particle.setColor(rgb[0], rgb[1], rgb[2]);
            particle.setAlphaPublic(0.62f);
            particle.setMaxAgePublic(maxAge);
            client.particleManager.addParticle(particle);
        }
    }

    private static void spawnRibbons(
        MinecraftClient client,
        ClientWorld world,
        SpriteProvider provider,
        double[] origin,
        float[] rgb,
        int count,
        int maxAge,
        double strength
    ) {
        if (provider == null) {
            return;
        }
        int ribbons = clamp(count / 3, 3, 8);
        for (int i = 0; i < ribbons; i++) {
            double angle = Math.PI * 2.0 * i / Math.max(1, ribbons);
            BongRibbonParticle particle = new BongRibbonParticle(
                world,
                origin[0] + Math.cos(angle) * 0.20,
                origin[1] + 0.55 + world.random.nextDouble() * 0.35,
                origin[2] + Math.sin(angle) * 0.20,
                -Math.sin(angle) * (0.045 + strength * 0.03),
                0.012,
                Math.cos(angle) * (0.045 + strength * 0.03)
            );
            particle.setRibbonWidth(0.10 + strength * 0.05, 0.018);
            particle.setSpritePublic(provider.getSprite(world.random));
            particle.setColor(rgb[0], rgb[1], rgb[2]);
            particle.setAlphaPublic(0.66f);
            particle.setMaxAgePublic(maxAge);
            client.particleManager.addParticle(particle);
        }
    }

    private static float[] rgb(int rgb) {
        return new float[] {
            ((rgb >> 16) & 0xFF) / 255f,
            ((rgb >> 8) & 0xFF) / 255f,
            (rgb & 0xFF) / 255f
        };
    }

    private static int clamp(int value, int min, int max) {
        return Math.max(min, Math.min(max, value));
    }

    private static Identifier id(String path) {
        return new Identifier("bong", path);
    }

    private enum Shape {
        SPRITE,
        LINE,
        RIBBON
    }

    private record Profile(Shape shape, int rgb, int count, int durationTicks, SpriteProvider provider) {
        static Profile forEvent(Identifier eventId) {
            if (HUO_XUE.equals(eventId)) return new Profile(Shape.SPRITE, 0xB22A2A, 12, 15, BongParticles.huoXueMistSprites);
            if (XU_GU.equals(eventId)) return new Profile(Shape.RIBBON, 0xE8F1FF, 9, 40, BongParticles.xuGuBandSprites);
            if (DUAN_XU.equals(eventId)) return new Profile(Shape.SPRITE, 0xC8A032, 12, 60, BongParticles.duanXuVortexSprites);
            if (TIE_BI.equals(eventId)) return new Profile(Shape.SPRITE, 0x8A8580, 10, 20, BongParticles.tieBiMetallicSprites);
            if (JIN_ZHONG.equals(eventId)) return new Profile(Shape.SPRITE, 0xF2C94C, 12, 30, BongParticles.jinZhongBellSprites);
            if (NING_JIA.equals(eventId)) return new Profile(Shape.SPRITE, 0x8FA09A, 8, 25, BongParticles.ningJiaCrustSprites);
            if (JI_FENG.equals(eventId)) return new Profile(Shape.LINE, 0x8FEA84, 8, 15, BongParticles.jiFengWindSprites);
            if (SUO_DI.equals(eventId)) return new Profile(Shape.LINE, 0xA050FF, 6, 8, BongParticles.suoDiArcSprites);
            if (HUI_LI.equals(eventId)) return new Profile(Shape.SPRITE, 0xF0C45C, 10, 30, BongParticles.huiLiBreathSprites);
            if (HU_GU.equals(eventId)) return new Profile(Shape.RIBBON, 0xF08A32, 9, 30, BongParticles.huGuStripeSprites);
            return null;
        }
    }
}
