package com.bong.client.combat.store;

/** Authoritative woliu vortex HUD state from {@code vortex_state}. */
public final class VortexStateStore {
    public record State(
        boolean active,
        float radius,
        float delta,
        float envQiAtCast,
        long maintainRemainingTicks,
        int interceptedCount,
        String activeSkillId,
        float chargeProgress,
        long cooldownUntilMs,
        String backfireLevel,
        float turbulenceRadius,
        float turbulenceIntensity,
        long turbulenceUntilMs
    ) {
        public State {
            activeSkillId = activeSkillId == null ? "" : activeSkillId;
            chargeProgress = Math.max(0f, Math.min(1f, chargeProgress));
            cooldownUntilMs = Math.max(0L, cooldownUntilMs);
            backfireLevel = backfireLevel == null ? "" : backfireLevel;
            turbulenceRadius = Math.max(0f, turbulenceRadius);
            turbulenceIntensity = Math.max(0f, Math.min(1f, turbulenceIntensity));
            turbulenceUntilMs = Math.max(0L, turbulenceUntilMs);
        }

        public static final State NONE = new State(false, 0f, 0f, 0f, 0L, 0, "", 0f, 0L, "", 0f, 0f, 0L);
    }

    private static volatile State snapshot = State.NONE;

    private VortexStateStore() {}

    public static State snapshot() { return snapshot; }

    public static void replace(State next) {
        snapshot = next == null ? State.NONE : next;
    }

    public static void resetForTests() {
        snapshot = State.NONE;
    }
}
