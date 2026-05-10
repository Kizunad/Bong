package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.OptionalInt;

/** TSY 容器搜刮开始/完成反馈。 */
public final class TsySearchFeedbackPlayer implements VfxPlayer {
    public static final Identifier DUST = new Identifier("bong", "tsy_search_dust");
    public static final Identifier LOOT_POP = new Identifier("bong", "tsy_search_loot_pop");

    private static final int DUST_RGB = 0x9A8974;
    private static final int LOOT_RGB = 0xFFD060;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) return;
        if (LOOT_POP.equals(payload.eventId())) {
            playLootPop(client, world, payload);
        } else {
            playDust(client, world, payload);
        }
    }

    private static void playDust(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload
    ) {
        double[] origin = payload.origin();
        int count = clamp(payload.count().orElse(OptionalInt.of(8).getAsInt()), 2, 32);
        int rgb = payload.colorRgb().orElse(DUST_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        for (int i = 0; i < count; i++) {
            double angle = world.random.nextDouble() * Math.PI * 2.0;
            double speed = 0.015 + world.random.nextDouble() * 0.035;
            BongSpriteParticle dust = new BongSpriteParticle(
                world,
                origin[0] + (world.random.nextDouble() - 0.5) * 0.8,
                origin[1] + 0.6 + world.random.nextDouble() * 0.5,
                origin[2] + (world.random.nextDouble() - 0.5) * 0.8,
                Math.cos(angle) * speed,
                0.01 + world.random.nextDouble() * 0.025,
                Math.sin(angle) * speed
            );
            dust.setColor(r, g, b);
            dust.setAlphaPublic(0.42f);
            dust.setScalePublic(0.75f);
            dust.setMaxAgePublic(24 + world.random.nextInt(16));
            if (BongParticles.enlightenmentDustSprites != null) {
                dust.setSpritePublic(BongParticles.enlightenmentDustSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(dust);
        }
    }

    private static void playLootPop(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload
    ) {
        double[] origin = payload.origin();
        double[] target = payload.direction().orElse(new double[] { origin[0], origin[1] + 1.6, origin[2] });
        int count = clamp(payload.count().orElse(OptionalInt.of(6).getAsInt()), 1, 24);
        int rgb = payload.colorRgb().orElse(LOOT_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        for (int i = 0; i < count; i++) {
            double jitter = (world.random.nextDouble() - 0.5) * 0.35;
            BongRibbonParticle pop = new BongRibbonParticle(
                world,
                origin[0] + jitter,
                origin[1] + 0.7 + world.random.nextDouble() * 0.3,
                origin[2] - jitter,
                (target[0] - origin[0]) * 0.055 + (world.random.nextDouble() - 0.5) * 0.025,
                0.08 + world.random.nextDouble() * 0.04,
                (target[2] - origin[2]) * 0.055 + (world.random.nextDouble() - 0.5) * 0.025
            );
            pop.setRibbonWidth(0.045, 0.012);
            pop.setColor(r, g, b);
            pop.setAlphaPublic(0.7f);
            pop.setMaxAgePublic(22 + world.random.nextInt(12));
            if (BongParticles.flyingSwordTrailSprites != null) {
                pop.setSpritePublic(BongParticles.flyingSwordTrailSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(pop);
        }
    }

    private static int clamp(int value, int min, int max) {
        return Math.max(min, Math.min(max, value));
    }
}
