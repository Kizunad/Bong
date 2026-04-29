package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.OptionalInt;

/**
 * {@code bong:realm_collapse_boundary} —— 域崩边界灰黑粒子场。
 *
 * <p>服务端用 {@code direction=[halfX, 0, halfZ]} 携带区域半尺寸，客户端沿矩形边界
 * 采样线形粒子。这里只做视觉边界，进入/撤离判定仍以 server zone AABB 为准。
 */
public final class RealmCollapseBoundaryPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "realm_collapse_boundary");

    private static final int FALLBACK_RGB = 0x2B2B31;
    private static final int DEFAULT_DURATION_TICKS = 160;
    private static final int DEFAULT_COUNT = 48;
    private static final int MAX_SEGMENTS = 64;
    private static final double MIN_HALF_EXTENT = 1.0;
    private static final double EDGE_DIRECTION_SPEED = 0.001;
    private static final double EDGE_Y_OFFSET = 0.08;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) return;

        double[] origin = payload.origin();
        double[] halfExtent = payload.direction()
            .orElse(new double[] { 16.0, 0.0, 16.0 });
        double halfX = Math.max(MIN_HALF_EXTENT, Math.abs(halfExtent[0]));
        double halfZ = Math.max(MIN_HALF_EXTENT, Math.abs(halfExtent[2]));
        int segments = clamp(payload.count().orElse(DEFAULT_COUNT), 4, MAX_SEGMENTS);
        int maxAge = payload.durationTicks().orElse(OptionalInt.of(DEFAULT_DURATION_TICKS).getAsInt());
        double strength = clamp(payload.strength().orElse(0.7), 0.0, 1.0);

        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        float alpha = (float) (0.32 + strength * 0.52);
        double halfWidth = 0.22 + strength * 0.38;

        for (int i = 0; i < segments; i++) {
            double t = (i + 0.5) / segments;
            EdgeSample sample = sampleRectangleEdge(origin[0], origin[1] + EDGE_Y_OFFSET, origin[2], halfX, halfZ, t);
            BongLineParticle particle = new BongLineParticle(
                world,
                sample.x,
                sample.y,
                sample.z,
                sample.dx * EDGE_DIRECTION_SPEED,
                0.0,
                sample.dz * EDGE_DIRECTION_SPEED
            );
            particle.setLineShape(1.0, 2.2, halfWidth);
            particle.setColor(r, g, b);
            particle.setAlphaPublic(alpha);
            particle.setMaxAgePublic(maxAge);
            if (BongParticles.swordQiTrailSprites != null) {
                particle.setSpritePublic(BongParticles.swordQiTrailSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    private static EdgeSample sampleRectangleEdge(
        double cx,
        double cy,
        double cz,
        double halfX,
        double halfZ,
        double ratio
    ) {
        double width = halfX * 2.0;
        double depth = halfZ * 2.0;
        double perimeter = (width + depth) * 2.0;
        double distance = perimeter * ratio;

        if (distance < width) {
            return new EdgeSample(cx - halfX + distance, cy, cz - halfZ, 1.0, 0.0);
        }
        distance -= width;
        if (distance < depth) {
            return new EdgeSample(cx + halfX, cy, cz - halfZ + distance, 0.0, 1.0);
        }
        distance -= depth;
        if (distance < width) {
            return new EdgeSample(cx + halfX - distance, cy, cz + halfZ, -1.0, 0.0);
        }
        distance -= width;
        return new EdgeSample(cx - halfX, cy, cz + halfZ - distance, 0.0, -1.0);
    }

    private static double clamp(double value, double min, double max) {
        return Math.max(min, Math.min(max, value));
    }

    private static int clamp(int value, int min, int max) {
        return Math.max(min, Math.min(max, value));
    }

    private record EdgeSample(double x, double y, double z, double dx, double dz) {
    }
}
