package com.bong.client.hud;

/** Client-local projection for Dugu v2 combat HUD surfaces. */
public final class DuguV2HudStateStore {
    public record State(
        boolean tainted,
        float taintIntensity,
        String taintHint,
        float revealRisk,
        float selfCurePercent,
        boolean selfRevealed,
        boolean shroudActive,
        long shroudUntilMs
    ) {
        public State {
            taintIntensity = clamp01(taintIntensity);
            taintHint = taintHint == null ? "" : taintHint;
            revealRisk = clamp01(revealRisk);
            selfCurePercent = Math.max(0f, Math.min(100f, selfCurePercent));
            shroudUntilMs = Math.max(0L, shroudUntilMs);
        }

        public static final State NONE = new State(false, 0f, "", 0f, 0f, false, false, 0L);
    }

    private static volatile State snapshot = State.NONE;

    private DuguV2HudStateStore() {
    }

    public static State snapshot() {
        return snapshot;
    }

    public static void replace(State next) {
        snapshot = next == null ? State.NONE : next;
    }

    public static void resetForTests() {
        snapshot = State.NONE;
    }

    private static float clamp01(float value) {
        if (!Float.isFinite(value)) return 0f;
        return Math.max(0f, Math.min(1f, value));
    }
}
