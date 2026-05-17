package com.bong.client.iris;

public final class BongShaderState {
    private static final float DEFAULT_LERP_SPEED = 0.1f;

    private static final float[] targets = new float[BongUniform.values().length];
    private static final float[] current = new float[BongUniform.values().length];
    private static final float[] lerpSpeeds = new float[BongUniform.values().length];
    private static final boolean[] overridden = new boolean[BongUniform.values().length];

    static {
        for (int i = 0; i < lerpSpeeds.length; i++) {
            lerpSpeeds[i] = DEFAULT_LERP_SPEED;
        }
    }

    private BongShaderState() {
    }

    public static void setTarget(BongUniform uniform, float value) {
        targets[uniform.ordinal()] = clamp(value, uniform);
    }

    public static void setOverride(BongUniform uniform, float value) {
        int idx = uniform.ordinal();
        overridden[idx] = true;
        current[idx] = clamp(value, uniform);
        targets[idx] = current[idx];
    }

    public static void clearOverride(BongUniform uniform) {
        overridden[uniform.ordinal()] = false;
    }

    public static void clearAllOverrides() {
        for (int i = 0; i < overridden.length; i++) {
            overridden[i] = false;
        }
    }

    public static boolean isOverridden(BongUniform uniform) {
        return overridden[uniform.ordinal()];
    }

    public static float get(BongUniform uniform) {
        return current[uniform.ordinal()];
    }

    public static float getTarget(BongUniform uniform) {
        return targets[uniform.ordinal()];
    }

    public static void setLerpSpeed(BongUniform uniform, float speed) {
        lerpSpeeds[uniform.ordinal()] = Math.max(0.001f, Math.min(1.0f, speed));
    }

    public static void tickInterpolate() {
        for (int i = 0; i < current.length; i++) {
            if (overridden[i]) {
                continue;
            }
            float diff = targets[i] - current[i];
            if (Math.abs(diff) < 0.001f) {
                current[i] = targets[i];
            } else {
                current[i] += diff * lerpSpeeds[i];
            }
        }
    }

    public static void reset() {
        for (int i = 0; i < current.length; i++) {
            targets[i] = 0f;
            current[i] = 0f;
            overridden[i] = false;
            lerpSpeeds[i] = DEFAULT_LERP_SPEED;
        }
    }

    private static float clamp01(float value) {
        return Math.max(0f, Math.min(1f, value));
    }

    private static float clamp(float value, BongUniform uniform) {
        if (Float.isNaN(value) || Float.isInfinite(value)) {
            return 0f;
        }
        if (uniform == BongUniform.WIND_ANGLE) {
            float twoPi = (float) (Math.PI * 2);
            return Math.max(0f, Math.min(twoPi, value));
        }
        return clamp01(value);
    }
}
