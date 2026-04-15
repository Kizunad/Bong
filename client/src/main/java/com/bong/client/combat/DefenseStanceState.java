package com.bong.client.combat;

/**
 * Current defense-stance + auxiliary indicators (fake-skin layers, vortex
 * cooldown) surfaced by the HUD (§3.4 / §11.1).
 */
public final class DefenseStanceState {
    public enum Stance { NONE, JIEMAI, TISHI, JUELING }

    private static final DefenseStanceState NONE = new DefenseStanceState(Stance.NONE, 0, false, 0L);

    private final Stance stance;
    private final int fakeSkinLayers;
    private final boolean vortexActive;
    private final long vortexReadyAtMs;

    private DefenseStanceState(Stance stance, int fakeSkinLayers, boolean vortexActive, long vortexReadyAtMs) {
        this.stance = stance == null ? Stance.NONE : stance;
        this.fakeSkinLayers = Math.max(0, fakeSkinLayers);
        this.vortexActive = vortexActive;
        this.vortexReadyAtMs = vortexReadyAtMs;
    }

    public static DefenseStanceState none() {
        return NONE;
    }

    public static DefenseStanceState of(Stance stance, int fakeSkinLayers, boolean vortexActive, long vortexReadyAtMs) {
        return new DefenseStanceState(stance, fakeSkinLayers, vortexActive, vortexReadyAtMs);
    }

    public Stance stance() { return stance; }
    public int fakeSkinLayers() { return fakeSkinLayers; }
    public boolean vortexActive() { return vortexActive; }
    public long vortexReadyAtMs() { return vortexReadyAtMs; }

    public boolean vortexOnCooldown(long nowMs) {
        return !vortexActive && vortexReadyAtMs > nowMs;
    }
}
