package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.OptionalInt;

/** TSY 进入 Collapsing/race-out 时的地裂与红尘爆发。 */
public final class TsyCollapseBurstPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "tsy_collapse_burst");

    private static final int DEFAULT_RGB = 0xFF3030;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) return;

        double[] origin = payload.origin();
        int count = clamp(payload.count().orElse(OptionalInt.of(20).getAsInt()), 8, 64);
        int maxAge = clamp(payload.durationTicks().orElse(OptionalInt.of(40).getAsInt()), 12, 120);
        double strength = clamp(payload.strength().orElse(0.85), 0.0, 1.0);
        int rgb = payload.colorRgb().orElse(DEFAULT_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;

        for (int i = 0; i < count; i++) {
            double angle = world.random.nextDouble() * Math.PI * 2.0;
            double radius = 1.0 + world.random.nextDouble() * (5.0 + strength * 7.0);
            double x = origin[0] + Math.cos(angle) * radius;
            double z = origin[2] + Math.sin(angle) * radius;
            BongGroundDecalParticle crack = new BongGroundDecalParticle(world, x, origin[1] + 0.05, z);
            crack.setDecalShape(0.35 + world.random.nextDouble() * 0.55, 0.035);
            crack.setSpin(angle, (world.random.nextDouble() - 0.5) * 0.018);
            crack.setColor(r, g, b);
            crack.setAlphaPublic((float) (0.34 + strength * 0.34));
            crack.setMaxAgePublic(maxAge);
            if (BongParticles.runeCharSprites != null) {
                crack.setSpritePublic(BongParticles.runeCharSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(crack);
        }

        for (int i = 0; i < Math.max(10, count / 2); i++) {
            double angle = world.random.nextDouble() * Math.PI * 2.0;
            double speed = 0.04 + world.random.nextDouble() * 0.06;
            BongSpriteParticle ember = new BongSpriteParticle(
                world,
                origin[0] + (world.random.nextDouble() - 0.5) * 2.0,
                origin[1] + 0.5 + world.random.nextDouble() * 1.5,
                origin[2] + (world.random.nextDouble() - 0.5) * 2.0,
                Math.cos(angle) * speed,
                0.02 + world.random.nextDouble() * 0.06,
                Math.sin(angle) * speed
            );
            ember.setColor(r, g, b);
            ember.setAlphaPublic(0.62f);
            ember.setScalePublic(0.72f);
            ember.setMaxAgePublic(18 + world.random.nextInt(18));
            if (BongParticles.qiAuraSprites != null) {
                ember.setSpritePublic(BongParticles.qiAuraSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(ember);
        }
    }

    private static double clamp(double value, double min, double max) {
        return Math.max(min, Math.min(max, value));
    }

    private static int clamp(int value, int min, int max) {
        return Math.max(min, Math.min(max, value));
    }
}
