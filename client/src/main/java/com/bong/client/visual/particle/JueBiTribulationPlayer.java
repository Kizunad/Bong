package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

/**
 * {@code bong:juebi_*} —— 绝壁劫 zone 级反震提示。
 *
 * <p>服务端已经写真实地形 overlay；客户端只补远距可见的暗环、地裂火星和冲天碎光。
 */
public final class JueBiTribulationPlayer implements VfxPlayer {
    public static final Identifier BOUNDARY = new Identifier("bong", "juebi_boundary");
    public static final Identifier FISSURE = new Identifier("bong", "juebi_fissure");
    public static final Identifier ERUPTION = new Identifier("bong", "juebi_eruption");

    private static final int DEFAULT_DURATION_TICKS = 180;
    private static final int DEFAULT_RGB = 0x140D18;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) return;

        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];
        int rgb = payload.colorRgb().orElse(DEFAULT_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        int duration = payload.durationTicks().orElse(DEFAULT_DURATION_TICKS);
        float strength = (float) Math.max(0.4, Math.min(1.6, payload.strength().orElse(1.0)));

        spawnDarkRing(client, world, ox, oy, oz, payload, r, g, b, duration, strength);
        spawnVerticalSparks(client, world, ox, oy, oz, payload, duration, strength);
    }

    private static void spawnDarkRing(
        MinecraftClient client,
        ClientWorld world,
        double ox,
        double oy,
        double oz,
        VfxEventPayload.SpawnParticle payload,
        float r,
        float g,
        float b,
        int duration,
        float strength
    ) {
        double radius = Math.max(24.0, Math.min(300.0, payload.direction().map(v -> Math.abs(v[0])).orElse(120.0)));
        BongGroundDecalParticle ring = new BongGroundDecalParticle(world, ox, oy, oz);
        ring.setDecalShape(radius, 0.08);
        ring.setSpin(world.random.nextDouble() * Math.PI * 2, -0.018 * strength);
        ring.setColor(r, g, b);
        ring.setAlphaPublic(Math.min(0.72f, 0.36f * strength));
        ring.setMaxAgePublic(duration);
        if (BongParticles.lingqiRippleSprites != null) {
            ring.setSpritePublic(BongParticles.lingqiRippleSprites.getSprite(world.random));
        }
        client.particleManager.addParticle(ring);
    }

    private static void spawnVerticalSparks(
        MinecraftClient client,
        ClientWorld world,
        double ox,
        double oy,
        double oz,
        VfxEventPayload.SpawnParticle payload,
        int duration,
        float strength
    ) {
        int count = Math.max(12, Math.min(96, payload.count().orElse(48)));
        for (int i = 0; i < count; i++) {
            double angle = world.random.nextDouble() * Math.PI * 2.0;
            double radius = 4.0 + world.random.nextDouble() * 22.0 * strength;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            BongLineParticle spark = new BongLineParticle(
                world,
                x,
                oy + world.random.nextDouble() * 2.0,
                z,
                (world.random.nextDouble() - 0.5) * 0.25,
                0.8 + world.random.nextDouble() * 1.8 * strength,
                (world.random.nextDouble() - 0.5) * 0.25
            );
            spark.setLineShape(0.8, 1.8 + strength, 0.18);
            spark.setColor(0.42f, 0.22f, 0.62f);
            spark.setAlphaPublic(Math.min(0.9f, 0.46f + strength * 0.18f));
            spark.setMaxAgePublic(Math.max(16, duration / 4));
            if (BongParticles.tribulationSparkSprites != null) {
                spark.setSpritePublic(BongParticles.tribulationSparkSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(spark);
        }
    }
}
