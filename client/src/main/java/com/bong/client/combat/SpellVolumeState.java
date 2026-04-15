package com.bong.client.combat;

/**
 * Local-only (non-networked) spell-volume scrub state for the right-bottom
 * conditional panel (§3.1 / §11.1).
 */
public final class SpellVolumeState {
    public static final float MIN_RADIUS = 0.3f;
    public static final float MAX_RADIUS = 5.0f;
    public static final float MIN_VELOCITY = 5.0f;
    public static final float MAX_VELOCITY = 80.0f;

    private static final SpellVolumeState IDLE = new SpellVolumeState(false, 1.5f, 20.0f, 0.5f);

    private final boolean visible;
    private final float radius;
    private final float velocityCap;
    private final float qiInvest; // [0,1]

    private SpellVolumeState(boolean visible, float radius, float velocityCap, float qiInvest) {
        this.visible = visible;
        this.radius = radius;
        this.velocityCap = velocityCap;
        this.qiInvest = qiInvest;
    }

    public static SpellVolumeState idle() {
        return IDLE;
    }

    public static SpellVolumeState visible(float radius, float velocityCap, float qiInvest) {
        return new SpellVolumeState(
            true,
            clamp(radius, MIN_RADIUS, MAX_RADIUS),
            clamp(velocityCap, MIN_VELOCITY, MAX_VELOCITY),
            clamp(qiInvest, 0.0f, 1.0f)
        );
    }

    public SpellVolumeState withRadius(float radius) {
        return new SpellVolumeState(true, clamp(radius, MIN_RADIUS, MAX_RADIUS), velocityCap, qiInvest);
    }

    public SpellVolumeState withVelocityCap(float velocityCap) {
        return new SpellVolumeState(true, radius, clamp(velocityCap, MIN_VELOCITY, MAX_VELOCITY), qiInvest);
    }

    public SpellVolumeState withQiInvest(float qi) {
        return new SpellVolumeState(visible, radius, velocityCap, clamp(qi, 0.0f, 1.0f));
    }

    public SpellVolumeState hidden() {
        return new SpellVolumeState(false, radius, velocityCap, qiInvest);
    }

    public boolean visible() { return visible; }
    public float radius() { return radius; }
    public float velocityCap() { return velocityCap; }
    public float qiInvest() { return qiInvest; }

    private static float clamp(float v, float lo, float hi) {
        if (Float.isNaN(v)) return lo;
        return Math.max(lo, Math.min(hi, v));
    }
}
