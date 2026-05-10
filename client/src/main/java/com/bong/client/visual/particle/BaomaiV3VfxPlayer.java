package com.bong.client.visual.particle;

import com.bong.client.combat.baomai.v3.BaomaiV3HudStateStore;
import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.Vec3d;

public final class BaomaiV3VfxPlayer implements VfxPlayer {
    public static final Identifier GROUND_WAVE_DUST = new Identifier("bong", "ground_wave_dust");
    public static final Identifier BLOOD_BURN_CRIMSON = new Identifier("bong", "blood_burn_crimson");
    public static final Identifier BODY_TRANSCENDENCE_PILLAR = new Identifier("bong", "body_transcendence_pillar");
    public static final Identifier MERIDIAN_RIPPLE_SCAR = new Identifier("bong", "meridian_ripple_scar");

    private static final int GROUND_RGB = 0xA8885A;
    private static final int BLOOD_RGB = 0xC0182B;
    private static final int TRANSCEND_RGB = 0xF5D36A;
    private static final int SCAR_RGB = 0xD8A03A;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) {
            return;
        }
        Identifier id = payload.eventId();
        if (GROUND_WAVE_DUST.equals(id)) {
            playGroundWave(client, payload);
        } else if (BLOOD_BURN_CRIMSON.equals(id)) {
            if (shouldUpdateLocalHud(client, payload)) {
                BaomaiV3HudStateStore.recordBloodBurn(payload.durationTicks().orElse(200));
            }
            playBurst(client, payload, BLOOD_RGB, 0.35, 0.9);
        } else if (BODY_TRANSCENDENCE_PILLAR.equals(id)) {
            if (shouldUpdateLocalHud(client, payload)) {
                BaomaiV3HudStateStore.recordBodyTranscendence(
                    payload.durationTicks().orElse(100),
                    10.0
                );
            }
            playPillar(client, payload);
        } else if (MERIDIAN_RIPPLE_SCAR.equals(id)) {
            if (shouldUpdateLocalHud(client, payload)) {
                BaomaiV3HudStateStore.recordMeridianRippleScar(payload.strength().orElse(0.45));
            }
            playBurst(client, payload, SCAR_RGB, 0.12, 0.45);
        }
    }

    private static boolean shouldUpdateLocalHud(
        MinecraftClient client,
        VfxEventPayload.SpawnParticle payload
    ) {
        if (client.player == null) {
            return false;
        }
        Vec3d playerPos = client.player.getPos();
        return isLocalPlayerOrigin(
            new double[] { playerPos.x, playerPos.y, playerPos.z },
            payload.origin()
        );
    }

    static boolean isLocalPlayerOrigin(double[] localPos, double[] origin) {
        if (localPos == null || origin == null || localPos.length != 3 || origin.length != 3) {
            return false;
        }
        double dx = localPos[0] - origin[0];
        double dy = localPos[1] - origin[1];
        double dz = localPos[2] - origin[2];
        return dx * dx + dy * dy + dz * dz <= 2.25;
    }

    private static void playGroundWave(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        double[] origin = payload.origin();
        int count = clamp(payload.count().orElse(28), 4, 64);
        int rgb = payload.colorRgb().orElse(GROUND_RGB);
        float[] color = rgb(rgb);
        double strength = payload.strength().orElse(1.0);
        int maxAge = payload.durationTicks().orElse(16);
        for (int i = 0; i < count; i++) {
            double angle = Math.PI * 2.0 * i / count;
            BongGroundDecalParticle particle = new BongGroundDecalParticle(
                world,
                origin[0] + Math.cos(angle) * 0.25,
                origin[1] + 0.03,
                origin[2] + Math.sin(angle) * 0.25
            ).setDecalShape(0.18 + 0.25 * strength, 0.03).setSpin(angle, 0.04);
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic(0.65f);
            particle.setMaxAgePublic(maxAge);
            if (BongParticles.lingqiRippleSprites != null) {
                particle.setSpritePublic(BongParticles.lingqiRippleSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    private static void playPillar(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        double[] origin = payload.origin();
        int count = clamp(payload.count().orElse(32), 8, 72);
        float[] color = rgb(payload.colorRgb().orElse(TRANSCEND_RGB));
        int maxAge = payload.durationTicks().orElse(24);
        for (int i = 0; i < count; i++) {
            double t = count == 1 ? 0.0 : (double) i / (count - 1);
            double angle = Math.PI * 8.0 * t;
            double radius = 0.25 + 0.65 * t;
            BongLineParticle particle = new BongLineParticle(
                world,
                origin[0] + Math.cos(angle) * radius,
                origin[1] + t * 2.6,
                origin[2] + Math.sin(angle) * radius,
                -Math.sin(angle) * 0.05,
                0.08,
                Math.cos(angle) * 0.05
            );
            particle.setLineShape(1.0, 0.9, 0.12);
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic(0.82f);
            particle.setMaxAgePublic(maxAge);
            if (BongParticles.tribulationSparkSprites != null) {
                particle.setSpritePublic(BongParticles.tribulationSparkSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    private static void playBurst(
        MinecraftClient client,
        VfxEventPayload.SpawnParticle payload,
        int fallbackRgb,
        double spread,
        double vertical
    ) {
        ClientWorld world = client.world;
        double[] origin = payload.origin();
        int count = clamp(payload.count().orElse(16), 2, 48);
        float[] color = rgb(payload.colorRgb().orElse(fallbackRgb));
        double strength = payload.strength().orElse(1.0);
        int maxAge = payload.durationTicks().orElse(12);
        for (int i = 0; i < count; i++) {
            double angle = Math.PI * 2.0 * i / count;
            BongLineParticle particle = new BongLineParticle(
                world,
                origin[0] + Math.cos(angle) * spread,
                origin[1] + 0.3 + (i % 5) * 0.08,
                origin[2] + Math.sin(angle) * spread,
                Math.cos(angle) * 0.04 * strength,
                vertical * 0.04,
                Math.sin(angle) * 0.04 * strength
            );
            particle.setLineShape(0.8, 0.6, 0.1 + 0.08 * strength);
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic(0.75f);
            particle.setMaxAgePublic(maxAge);
            if (BongParticles.qiAuraSprites != null) {
                particle.setSpritePublic(BongParticles.qiAuraSprites.getSprite(world.random));
            }
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

    private static int clamp(int value, int lo, int hi) {
        return Math.max(lo, Math.min(hi, value));
    }
}
