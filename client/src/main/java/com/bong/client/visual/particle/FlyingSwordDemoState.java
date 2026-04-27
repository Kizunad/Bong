package com.bong.client.visual.particle;

import net.minecraft.util.math.Vec3d;

/**
 * Pure state for the flying-sword VFX demo. The real authoritative entity will
 * arrive from server-side combat/world plans; this class only drives the local
 * demo renderer and ribbon trail without sending per-tick vfx events.
 */
final class FlyingSwordDemoState {
    private static final Vec3d FALLBACK_DIRECTION = new Vec3d(1.0, 0.0, 0.0);

    final Vec3d origin;
    final Vec3d direction;
    final int maxAgeTicks;
    final double speedPerTick;
    final double bobAmplitude;
    final int colorRgb;
    final double strength;
    int ageTicks;

    FlyingSwordDemoState(
        Vec3d origin,
        Vec3d direction,
        int maxAgeTicks,
        double strength,
        int colorRgb
    ) {
        this.origin = origin;
        this.direction = normalizeDirection(direction);
        this.maxAgeTicks = clamp(maxAgeTicks, 10, 200);
        this.strength = clamp(strength, 0.0, 1.0);
        this.speedPerTick = 0.12 + 0.08 * this.strength;
        this.bobAmplitude = 0.10 + 0.10 * this.strength;
        this.colorRgb = colorRgb;
        this.ageTicks = 0;
    }

    Vec3d position(float tickDelta) {
        double t = ageTicks + tickDelta;
        double bob = Math.sin(t * 0.35) * bobAmplitude;
        return origin.add(direction.multiply(t * speedPerTick)).add(0.0, bob, 0.0);
    }

    Vec3d previousPosition() {
        double t = Math.max(0, ageTicks - 1);
        double bob = Math.sin(t * 0.35) * bobAmplitude;
        return origin.add(direction.multiply(t * speedPerTick)).add(0.0, bob, 0.0);
    }

    boolean tick() {
        ageTicks++;
        return ageTicks < maxAgeTicks;
    }

    static Vec3d normalizeDirection(Vec3d direction) {
        if (direction == null || !isFinite(direction) || direction.lengthSquared() < 1.0e-8) {
            return FALLBACK_DIRECTION;
        }
        return direction.normalize();
    }

    private static boolean isFinite(Vec3d v) {
        return Double.isFinite(v.x) && Double.isFinite(v.y) && Double.isFinite(v.z);
    }

    private static int clamp(int value, int lo, int hi) {
        return Math.max(lo, Math.min(hi, value));
    }

    private static double clamp(double value, double lo, double hi) {
        if (!Double.isFinite(value)) {
            return lo;
        }
        return Math.max(lo, Math.min(hi, value));
    }
}
