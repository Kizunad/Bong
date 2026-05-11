package com.bong.client.season;

import com.bong.client.state.SeasonState;
import com.bong.client.visual.particle.BongGroundDecalParticle;
import com.bong.client.visual.particle.BongLineParticle;
import com.bong.client.visual.particle.BongParticles;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.particle.ParticleTypes;
import net.minecraft.util.math.random.Random;

import java.util.ArrayList;
import java.util.List;

public final class SeasonParticleEmitter {
    private SeasonParticleEmitter() {
    }

    public static List<ParticleCue> plan(SeasonState state, long worldTick) {
        if (state == null) {
            return List.of();
        }
        List<ParticleCue> cues = new ArrayList<>();
        switch (state.phase()) {
            case SUMMER -> {
                if (worldTick % 10L == 0L) {
                    cues.add(new ParticleCue(ParticleKind.HEAT_SHIMMER, "lingqi_ripple", 1, 0xFFD700, 0.15f, 30, 0.5, 0.015));
                }
                if (worldTick % 120L == 0L) {
                    cues.add(new ParticleCue(ParticleKind.DISTANT_THUNDER_FLASH, "tribulation_spark", 1, 0xFFFFFF, 0.80f, 2, 8.0, 0.0));
                }
                if (worldTick % 20L == 0L) {
                    cues.add(new ParticleCue(ParticleKind.BOTANY_EVAPORATION, "enlightenment_dust", 1, 0xFFD36A, 0.35f, 20, 0.25, 0.035));
                }
            }
            case WINTER -> {
                if (worldTick % 10L == 0L) {
                    cues.add(new ParticleCue(ParticleKind.SNOW_DRIFT, "cloud256_dust", 2, 0xFFFFFF, 0.55f, 45, 0.20, -0.03));
                }
                if (worldTick % 60L == 0L) {
                    cues.add(new ParticleCue(ParticleKind.ICE_SPARK, "enlightenment_dust", 1, 0xC0E0FF, 0.60f, 15, 0.18, 0.01));
                }
            }
            case SUMMER_TO_WINTER, WINTER_TO_SUMMER -> {
                if (worldTick % 40L == 0L) {
                    cues.add(new ParticleCue(ParticleKind.CHAOTIC_QI_LINE, "sword_qi_trail", 1, 0x9966CC, 0.20f, 10, 2.0, 0.0));
                }
                if (worldTick % 20L == 0L) {
                    cues.add(new ParticleCue(ParticleKind.TRIBULATION_MARK, "tribulation_spark", 1, 0xFF4444, 0.45f, 20, 0.20, 0.02));
                }
            }
        }
        return List.copyOf(cues);
    }

    public static void updateSeason(MinecraftClient client, SeasonState state, long worldTick) {
        ClientWorld world = client == null ? null : client.world;
        if (world == null || client.player == null) {
            return;
        }
        Random random = world.random;
        for (ParticleCue cue : plan(state, worldTick)) {
            for (int i = 0; i < cue.count(); i++) {
                double x = client.player.getX() + (random.nextDouble() - 0.5) * 10.0;
                double y = client.player.getY() + 0.5 + random.nextDouble() * 2.4;
                double z = client.player.getZ() + (random.nextDouble() - 0.5) * 10.0;
                spawnCue(client, world, cue, x, y, z, random);
            }
        }
    }

    private static void spawnCue(
        MinecraftClient client,
        ClientWorld world,
        ParticleCue cue,
        double x,
        double y,
        double z,
        Random random
    ) {
        float[] rgb = rgb(cue.tintRgb());
        switch (cue.kind()) {
            case SNOW_DRIFT -> world.addParticle(ParticleTypes.SNOWFLAKE, x, y, z, 0.0, cue.yVelocity(), 0.0);
            case CHAOTIC_QI_LINE -> {
                if (client.particleManager == null) {
                    world.addParticle(BongParticles.SWORD_QI_TRAIL, x, y, z, 0.0, cue.yVelocity(), 0.0);
                } else {
                    double dx = (random.nextDouble() - 0.5) * 0.16;
                    double dy = (random.nextDouble() - 0.5) * 0.08;
                    double dz = (random.nextDouble() - 0.5) * 0.16;
                    BongLineParticle particle = new BongLineParticle(world, x, y, z, dx, dy, dz);
                    particle.setLineShape(cue.scale(), cue.scale() * 1.6, 0.10);
                    particle.setColor(rgb[0], rgb[1], rgb[2]);
                    particle.setAlphaPublic(cue.alpha());
                    particle.setMaxAgePublic(cue.lifetimeTicks());
                    if (BongParticles.swordQiTrailSprites != null) {
                        particle.setSpritePublic(BongParticles.swordQiTrailSprites.getSprite(random));
                    }
                    client.particleManager.addParticle(particle);
                }
            }
            case HEAT_SHIMMER -> spawnHeatDecal(client, world, cue, x, y - 0.45, z, rgb, random);
            case DISTANT_THUNDER_FLASH, TRIBULATION_MARK ->
                world.addParticle(BongParticles.TRIBULATION_SPARK, x, y, z, 0.0, cue.yVelocity(), 0.0);
            case BOTANY_EVAPORATION, ICE_SPARK ->
                world.addParticle(BongParticles.ENLIGHTENMENT_DUST, x, y, z, 0.0, cue.yVelocity(), 0.0);
        }
    }

    private static void spawnHeatDecal(
        MinecraftClient client,
        ClientWorld world,
        ParticleCue cue,
        double x,
        double y,
        double z,
        float[] rgb,
        Random random
    ) {
        if (client.particleManager == null || BongParticles.lingqiRippleSprites == null) {
            world.addParticle(BongParticles.LINGQI_RIPPLE, x, y, z, 0.0, cue.yVelocity(), 0.0);
            return;
        }
        BongGroundDecalParticle particle = new BongGroundDecalParticle(world, x, y, z);
        particle.setDecalShape(cue.scale(), 0.03);
        particle.setSpin(random.nextDouble() * Math.PI * 2.0, 0.025);
        particle.setSpritePublic(BongParticles.lingqiRippleSprites.getSprite(random));
        particle.setColor(rgb[0], rgb[1], rgb[2]);
        particle.setAlphaPublic(cue.alpha());
        particle.setMaxAgePublic(cue.lifetimeTicks());
        client.particleManager.addParticle(particle);
    }

    private static float[] rgb(int rgb) {
        return new float[] {
            ((rgb >> 16) & 0xFF) / 255.0f,
            ((rgb >> 8) & 0xFF) / 255.0f,
            (rgb & 0xFF) / 255.0f,
        };
    }

    public enum ParticleKind {
        HEAT_SHIMMER,
        DISTANT_THUNDER_FLASH,
        BOTANY_EVAPORATION,
        SNOW_DRIFT,
        ICE_SPARK,
        CHAOTIC_QI_LINE,
        TRIBULATION_MARK
    }

    public record ParticleCue(
        ParticleKind kind,
        String spriteId,
        int count,
        int tintRgb,
        float alpha,
        int lifetimeTicks,
        double scale,
        double yVelocity
    ) {
        public ParticleCue {
            if (kind == null) {
                throw new IllegalArgumentException("kind must not be null");
            }
            spriteId = spriteId == null ? "" : spriteId.trim();
            count = Math.max(0, count);
            tintRgb &= 0xFFFFFF;
            alpha = Math.max(0.0f, Math.min(1.0f, alpha));
            lifetimeTicks = Math.max(1, lifetimeTicks);
            scale = Double.isFinite(scale) ? Math.max(0.01, scale) : 0.01;
            yVelocity = Double.isFinite(yVelocity) ? yVelocity : 0.0;
        }
    }
}
