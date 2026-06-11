package com.bong.client.visual.particle;

import com.bong.client.network.QiAttritionPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.particle.SpriteProvider;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.ArrayList;
import java.util.List;

public final class QiAttritionVfxPlayer {
    public static final Identifier CHANNEL = new Identifier("bong", "vfx/qi_attrition");
    public static final int COLOR_RGB = 0xD4A820;
    public static final int LIFETIME_TICKS = 8;
    public static final int MIN_PARTICLES = 3;
    public static final int MAX_PARTICLES = 5;
    public static final double UPWARD_DRIFT_METERS = 0.5;

    private static final double VERTICAL_VELOCITY = UPWARD_DRIFT_METERS / LIFETIME_TICKS;
    private static final double BASE_RADIUS = 0.08;
    private static final double OUTWARD_VELOCITY = 0.018;

    private QiAttritionVfxPlayer() {
    }

    public static List<ParticleSpec> plan(QiAttritionPayload payload) {
        double[] pos = payload.worldPos();
        int count = particleCount(payload.itemEntityId());
        double seedAngle = Math.floorMod(payload.itemEntityId(), 360L) * Math.PI / 180.0;

        List<ParticleSpec> specs = new ArrayList<>(count);
        for (int i = 0; i < count; i++) {
            double angle = seedAngle + (Math.PI * 2.0 * i / count);
            double radius = BASE_RADIUS + 0.025 * i;
            double cos = Math.cos(angle);
            double sin = Math.sin(angle);
            specs.add(new ParticleSpec(
                pos[0] + cos * radius,
                pos[1] + 0.15 + i * 0.025,
                pos[2] + sin * radius,
                cos * OUTWARD_VELOCITY,
                VERTICAL_VELOCITY,
                sin * OUTWARD_VELOCITY,
                COLOR_RGB,
                LIFETIME_TICKS,
                UPWARD_DRIFT_METERS
            ));
        }
        return specs;
    }

    public static void play(MinecraftClient client, QiAttritionPayload payload) {
        if (client == null || payload == null || client.particleManager == null) {
            return;
        }
        ClientWorld world = client.world;
        if (world == null) {
            return;
        }
        SpriteProvider sprites = BongParticles.qiAuraSprites != null
            ? BongParticles.qiAuraSprites
            : BongParticles.enlightenmentDustSprites;
        if (sprites == null) {
            return;
        }

        float r = ((COLOR_RGB >> 16) & 0xFF) / 255f;
        float g = ((COLOR_RGB >> 8) & 0xFF) / 255f;
        float b = (COLOR_RGB & 0xFF) / 255f;
        for (ParticleSpec spec : plan(payload)) {
            EnlightenmentAuraPlayer.spawnSprite(
                client,
                world,
                sprites,
                spec.x(),
                spec.y(),
                spec.z(),
                spec.vx(),
                spec.vy(),
                spec.vz(),
                r,
                g,
                b,
                0.82f,
                spec.lifetimeTicks(),
                0.055f
            );
        }
    }

    private static int particleCount(long itemEntityId) {
        return MIN_PARTICLES + (int) Math.floorMod(itemEntityId, MAX_PARTICLES - MIN_PARTICLES + 1L);
    }

    public record ParticleSpec(
        double x,
        double y,
        double z,
        double vx,
        double vy,
        double vz,
        int colorRgb,
        int lifetimeTicks,
        double upwardDriftMeters
    ) {
    }
}
