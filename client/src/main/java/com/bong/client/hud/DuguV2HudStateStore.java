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
        long shroudUntilMs,
        // plan-combat-skill-feedback-bridges-v1 P5：永久真元上限衰减 HUD 字段
        /** 本次衰减量（0=无衰减事件，>0=正在闪烁窗口内）。§视听精度文档待人工补 */
        float qiMaxDecayLoss,
        /** 衰减后真元上限（用于显示绝对值）。 */
        float qiMaxAfter,
        /** 衰减闪烁窗口截止时刻（ms）；nowMillis < decayExpiryMs 时显示衰减条。 */
        long decayExpiryMs
    ) {
        public State {
            taintIntensity = clamp01(taintIntensity);
            taintHint = taintHint == null ? "" : taintHint;
            revealRisk = clamp01(revealRisk);
            selfCurePercent = Math.max(0f, Math.min(100f, selfCurePercent));
            shroudUntilMs = Math.max(0L, shroudUntilMs);
            qiMaxDecayLoss = Math.max(0f, qiMaxDecayLoss);
            qiMaxAfter = Math.max(0f, qiMaxAfter);
            decayExpiryMs = Math.max(0L, decayExpiryMs);
        }

        public static final State NONE = new State(false, 0f, "", 0f, 0f, false, false, 0L, 0f, 0f, 0L);
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
