package com.bong.client.visual.particle;

import net.minecraft.util.math.Vec3d;

/** Client-local demo state for a persistent formation core / barrier-node style VFX. */
final class FormationCoreDemoState {
    final Vec3d origin;
    final int maxAgeTicks;
    final double strength;
    final int colorRgb;
    int ageTicks;

    FormationCoreDemoState(Vec3d origin, int maxAgeTicks, double strength, int colorRgb) {
        this.origin = origin;
        this.maxAgeTicks = clamp(maxAgeTicks, 20, 240);
        this.strength = clamp(strength, 0.0, 1.0);
        this.colorRgb = colorRgb;
        this.ageTicks = 0;
    }

    boolean shouldPulse() {
        return ageTicks % 16 == 0;
    }

    int pulseAgeTicks() {
        return Math.min(36, maxAgeTicks - ageTicks + 1);
    }

    double halfSize() {
        return 1.35 + 1.15 * strength;
    }

    boolean tick() {
        ageTicks++;
        return ageTicks < maxAgeTicks;
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
