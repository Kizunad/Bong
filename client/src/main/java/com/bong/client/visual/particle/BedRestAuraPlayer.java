package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class BedRestAuraPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "furniture_bed_rest");

    private static final int FALLBACK_RGB = 0xE8C97A;
    private static final int DEFAULT_COUNT = 6;
    private static final int DEFAULT_MAX_AGE = 12;
    private static final double DEFAULT_STRENGTH = 0.75;
    private static final float MIN_ALPHA = 0.45f;
    private static final float MAX_ALPHA = 0.9f;
    private static final int MIN_COUNT = 1;
    private static final int MAX_COUNT = 16;
    private static final int MIN_MAX_AGE = 1;
    private static final int MAX_MAX_AGE = 40;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) return;

        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];
        FurnitureAuraParticleSpec spec = resolveSpec(payload);

        for (int i = 0; i < spec.count(); i++) {
            double dx = (world.random.nextDouble() - 0.5) * 0.9;
            double dy = 0.2 + world.random.nextDouble() * 0.7;
            double dz = (world.random.nextDouble() - 0.5) * 0.9;
            double vx = (world.random.nextDouble() - 0.5) * 0.025;
            double vy = 0.018 + world.random.nextDouble() * 0.018;
            double vz = (world.random.nextDouble() - 0.5) * 0.025;
            EnlightenmentAuraPlayer.spawnSprite(client, world, BongParticles.enlightenmentDustSprites,
                ox + dx, oy + dy, oz + dz, vx, vy, vz, spec.red(), spec.green(), spec.blue(),
                spec.alpha(), spec.maxAge(), 0.06f);
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
