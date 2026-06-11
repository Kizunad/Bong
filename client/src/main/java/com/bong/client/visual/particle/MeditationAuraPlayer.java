package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class MeditationAuraPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "furniture_meditation");

    private static final int FALLBACK_RGB = 0xBFD8C8;
    private static final int DEFAULT_COUNT = 8;
    private static final int DEFAULT_MAX_AGE = 16;
    private static final double DEFAULT_STRENGTH = 0.75;
    private static final float MIN_ALPHA = 0.4f;
    private static final float MAX_ALPHA = 0.85f;
    private static final int MIN_COUNT = 1;
    private static final int MAX_COUNT = 24;
    private static final int MIN_MAX_AGE = 1;
    private static final int MAX_MAX_AGE = 48;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) return;

        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];
        FurnitureAuraParticleSpec spec = resolveSpec(payload);
        double phase = world.random.nextDouble() * Math.PI * 2.0;

        for (int i = 0; i < spec.count(); i++) {
            double angle = phase + (Math.PI * 2.0 * i / spec.count());
            double radius = 0.45 + world.random.nextDouble() * 0.18;
            double x = ox + Math.cos(angle) * radius;
            double y = oy + 0.25 + world.random.nextDouble() * 0.55;
            double z = oz + Math.sin(angle) * radius;
            double vx = -Math.sin(angle) * 0.012;
            double vy = 0.008 + world.random.nextDouble() * 0.012;
            double vz = Math.cos(angle) * 0.012;
            EnlightenmentAuraPlayer.spawnSprite(client, world, BongParticles.enlightenmentDustSprites,
                x, y, z, vx, vy, vz, spec.red(), spec.green(), spec.blue(), spec.alpha(),
                spec.maxAge(), 0.065f);
        }
    }

    static FurnitureAuraParticleSpec resolveSpec(VfxEventPayload.SpawnParticle payload) {
        float[] rgb = GameplayVfxUtil.rgb(payload, FALLBACK_RGB);
        float alpha = (float) GameplayVfxUtil.clamp(
            payload.strength().orElse(DEFAULT_STRENGTH),
            MIN_ALPHA,
            MAX_ALPHA
        );
        return new FurnitureAuraParticleSpec(
            rgb[0],
            rgb[1],
            rgb[2],
            alpha,
            GameplayVfxUtil.clamp(payload.count().orElse(DEFAULT_COUNT), MIN_COUNT, MAX_COUNT),
            GameplayVfxUtil.clamp(payload.durationTicks().orElse(DEFAULT_MAX_AGE), MIN_MAX_AGE, MAX_MAX_AGE)
        );
    }
}
