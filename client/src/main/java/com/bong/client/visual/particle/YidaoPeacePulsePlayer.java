package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

/** 医道平和色治疗脉络：5 招共用轻量粒子 player，差异由 event id 和 payload count 控制。 */
public final class YidaoPeacePulsePlayer implements VfxPlayer {
    public static final Identifier MERIDIAN_REPAIR = new Identifier("bong", "yidao_meridian_repair");
    public static final Identifier CONTAM_PURGE = new Identifier("bong", "yidao_contam_purge");
    public static final Identifier EMERGENCY_RESUSCITATE = new Identifier("bong", "yidao_emergency_resuscitate");
    public static final Identifier LIFE_EXTENSION = new Identifier("bong", "yidao_life_extension");
    public static final Identifier MASS_MERIDIAN_REPAIR = new Identifier("bong", "yidao_mass_meridian_repair");

    private static final int FALLBACK_RGB = 0xA8E6CF;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) return;

        double[] origin = payload.origin();
        if (origin.length != 3 || !finiteVec3(origin)) return;
        double ox = origin[0];
        double oy = origin[1];
        double oz = origin[2];
        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        float alpha = (float) Math.max(0.35, Math.min(0.9, payload.strength().orElse(0.75)));
        int count = clamp(payload.count().orElse(defaultCount(payload.eventId())), 1, 64);
        int maxAge = clamp(payload.durationTicks().orElse(50), 1, 200);
        boolean mass = MASS_MERIDIAN_REPAIR.equals(payload.eventId());

        for (int i = 0; i < count; i++) {
            double angle = (Math.PI * 2.0 * i / count) + world.random.nextDouble() * 0.25;
            double radius = mass ? 1.8 + world.random.nextDouble() * 1.2 : 0.35 + world.random.nextDouble() * 0.55;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + world.random.nextDouble() * (mass ? 1.2 : 0.7);
            double vx = (ox - x) * 0.015;
            double vy = 0.012 + world.random.nextDouble() * 0.018;
            double vz = (oz - z) * 0.015;
            EnlightenmentAuraPlayer.spawnSprite(
                client,
                world,
                BongParticles.enlightenmentDustSprites,
                x,
                y,
                z,
                vx,
                vy,
                vz,
                r,
                g,
                b,
                alpha,
                maxAge,
                mass ? 0.18f : 0.11f
            );
        }
    }

    private static int defaultCount(Identifier id) {
        if (MASS_MERIDIAN_REPAIR.equals(id)) return 24;
        if (LIFE_EXTENSION.equals(id)) return 18;
        if (EMERGENCY_RESUSCITATE.equals(id)) return 10;
        return 12;
    }

    private static int clamp(int value, int min, int max) {
        return Math.max(min, Math.min(max, value));
    }

    private static boolean finiteVec3(double[] values) {
        return Double.isFinite(values[0]) && Double.isFinite(values[1]) && Double.isFinite(values[2]);
    }
}
