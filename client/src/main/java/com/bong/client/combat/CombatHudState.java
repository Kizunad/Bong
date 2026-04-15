package com.bong.client.combat;

import java.util.Objects;

/**
 * Lightweight per-frame combat HUD state snapshot (§2.1 / §11.1).
 *
 * <p>No raw hp/qi numbers — only percentages and boolean flags. Consumed by the
 * left-bottom mini-body control, edge-feedback pulses, and DerivedAttr short
 * displays.
 */
public final class CombatHudState {
    private static final CombatHudState EMPTY = new CombatHudState(1.0f, 1.0f, 1.0f, DerivedAttrFlags.none(), false);

    private final float hpPercent;
    private final float qiPercent;
    private final float staminaPercent;
    private final DerivedAttrFlags derived;
    private final boolean active;

    private CombatHudState(
        float hpPercent,
        float qiPercent,
        float staminaPercent,
        DerivedAttrFlags derived,
        boolean active
    ) {
        this.hpPercent = hpPercent;
        this.qiPercent = qiPercent;
        this.staminaPercent = staminaPercent;
        this.derived = Objects.requireNonNull(derived, "derived");
        this.active = active;
    }

    public static CombatHudState empty() {
        return EMPTY;
    }

    public static CombatHudState create(
        float hpPercent,
        float qiPercent,
        float staminaPercent,
        DerivedAttrFlags derived
    ) {
        return new CombatHudState(
            clamp01(hpPercent),
            clamp01(qiPercent),
            clamp01(staminaPercent),
            derived == null ? DerivedAttrFlags.none() : derived,
            true
        );
    }

    private static float clamp01(float v) {
        if (Float.isNaN(v)) return 0.0f;
        if (v < 0.0f) return 0.0f;
        if (v > 1.0f) return 1.0f;
        return v;
    }

    public float hpPercent() {
        return hpPercent;
    }

    public float qiPercent() {
        return qiPercent;
    }

    public float staminaPercent() {
        return staminaPercent;
    }

    public DerivedAttrFlags derived() {
        return derived;
    }

    public boolean active() {
        return active;
    }
}
