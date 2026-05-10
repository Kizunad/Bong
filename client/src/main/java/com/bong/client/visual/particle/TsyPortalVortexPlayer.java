package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.OptionalInt;

/** TSY 裂缝三态粒子：主裂缝、深层缝、race-out 塌缩裂口。 */
public final class TsyPortalVortexPlayer implements VfxPlayer {
    public static final Identifier MAIN_RIFT = new Identifier("bong", "tsy_portal_idle");
    public static final Identifier DEEP_RIFT = new Identifier("bong", "tsy_portal_deep");
    public static final Identifier COLLAPSE_TEAR = new Identifier("bong", "tsy_portal_tear");

    private static final int MAIN_RGB = 0x6644AA;
    private static final int DEEP_RGB = 0xAA2222;
    private static final int TEAR_RGB = 0xF8F6FF;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) return;

        double[] origin = payload.origin();
        Identifier eventId = payload.eventId();
        boolean tear = COLLAPSE_TEAR.equals(eventId);
        int count = clamp(payload.count().orElse(OptionalInt.of(tear ? 18 : 16).getAsInt()), 4, 48);
        int maxAge = clamp(payload.durationTicks().orElse(OptionalInt.of(tear ? 18 : 64).getAsInt()), 8, 160);
        double strength = clamp(payload.strength().orElse(0.7), 0.0, 1.0);
        int rgb = payload.colorRgb().orElse(defaultColor(eventId));
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        double spinSpeed = DEEP_RIFT.equals(eventId) ? 1.5 : 1.0;

        for (int i = 0; i < count; i++) {
            double angle = Math.PI * 2.0 * i / count + world.random.nextDouble() * 0.18;
            double radius = tear ? 0.2 + world.random.nextDouble() * 1.2 : 0.75 + world.random.nextDouble() * 0.55;
            double jitter = tear ? (world.random.nextDouble() - 0.5) * 0.6 : 0.0;
            double x = origin[0] + Math.cos(angle) * radius + jitter;
            double y = origin[1] + 0.8 + (world.random.nextDouble() - 0.5) * (tear ? 1.0 : 0.6);
            double z = origin[2] + Math.sin(angle) * radius + jitter;
            double tangentX = -Math.sin(angle) * 0.035 * spinSpeed;
            double tangentZ = Math.cos(angle) * 0.035 * spinSpeed;
            double inwardX = (origin[0] - x) * 0.018;
            double inwardZ = (origin[2] - z) * 0.018;

            BongRibbonParticle ribbon = new BongRibbonParticle(
                world,
                x,
                y,
                z,
                tangentX + inwardX,
                (world.random.nextDouble() - 0.5) * 0.018,
                tangentZ + inwardZ
            );
            ribbon.setRibbonWidth(0.08 + strength * 0.05, 0.015);
            ribbon.setColor(r, g, b);
            ribbon.setAlphaPublic((float) (tear ? 0.72 : 0.38 + strength * 0.32));
            ribbon.setMaxAgePublic(tear ? 10 + world.random.nextInt(Math.max(1, maxAge - 9)) : maxAge);
            if (BongParticles.flyingSwordTrailSprites != null) {
                ribbon.setSpritePublic(BongParticles.flyingSwordTrailSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(ribbon);
        }

        if (tear) {
            spawnTearFlash(client, world, origin, r, g, b);
        }
    }

    private static void spawnTearFlash(
        MinecraftClient client,
        ClientWorld world,
        double[] origin,
        float r,
        float g,
        float b
    ) {
        for (int i = 0; i < 8; i++) {
            double angle = world.random.nextDouble() * Math.PI * 2.0;
            double speed = 0.04 + world.random.nextDouble() * 0.06;
            BongSpriteParticle spark = new BongSpriteParticle(
                world,
                origin[0] + (world.random.nextDouble() - 0.5) * 0.4,
                origin[1] + 0.7 + world.random.nextDouble() * 0.8,
                origin[2] + (world.random.nextDouble() - 0.5) * 0.4,
                Math.cos(angle) * speed,
                (world.random.nextDouble() - 0.2) * speed,
                Math.sin(angle) * speed
            );
            spark.setColor(r, g, b);
            spark.setAlphaPublic(0.86f);
            spark.setScalePublic(0.55f);
            spark.setMaxAgePublic(12 + world.random.nextInt(9));
            if (BongParticles.qiAuraSprites != null) {
                spark.setSpritePublic(BongParticles.qiAuraSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(spark);
        }
    }

    private static int defaultColor(Identifier eventId) {
        if (DEEP_RIFT.equals(eventId)) return DEEP_RGB;
        if (COLLAPSE_TEAR.equals(eventId)) return TEAR_RGB;
        return MAIN_RGB;
    }

    private static double clamp(double value, double min, double max) {
        return Math.max(min, Math.min(max, value));
    }

    private static int clamp(int value, int min, int max) {
        return Math.max(min, Math.min(max, value));
    }
}
