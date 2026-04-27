package com.bong.client.combat.store;

import java.util.Collections;
import java.util.List;
import java.util.Objects;

/**
 * Full-screen death overlay state (plan §U3 / §2.3).
 * Populated by {@link com.bong.client.combat.handler.DeathScreenHandler} when
 * the server announces {@code death_screen}.
 */
public final class DeathStateStore {

    public record State(
        boolean visible,
        String cause,              // "dao_heart_shatter" / "pk" / "tribulation" / ...
        float luckRemaining,       // 重生概率 0..1
        List<String> finalWords,   // 遗念
        long countdownUntilMs,     // revive deadline
        boolean canReincarnate,
        boolean canTerminate,
        String stage,              // "fortune" / "tribulation"
        int deathNumber,
        String zoneKind,           // "ordinary" / "death" / "negative"
        double yearsLived,
        int lifespanCapByRealm,
        double remainingYears,
        int deathPenaltyYears,
        double lifespanTickRateMultiplier,
        boolean windCandle
    ) {
        public State {
            cause = cause == null ? "" : cause;
            finalWords = finalWords == null ? Collections.emptyList() : List.copyOf(finalWords);
            stage = stage == null ? "" : stage;
            zoneKind = zoneKind == null ? "" : zoneKind;
            if (Float.isNaN(luckRemaining) || luckRemaining < 0f) luckRemaining = 0f;
            if (luckRemaining > 1f) luckRemaining = 1f;
            deathNumber = Math.max(0, deathNumber);
            yearsLived = Math.max(0.0, yearsLived);
            lifespanCapByRealm = Math.max(0, lifespanCapByRealm);
            remainingYears = Math.max(0.0, remainingYears);
            deathPenaltyYears = Math.max(0, deathPenaltyYears);
            lifespanTickRateMultiplier = Math.max(0.0, lifespanTickRateMultiplier);
        }

        public State(boolean visible, String cause, float luckRemaining, List<String> finalWords,
                     long countdownUntilMs, boolean canReincarnate, boolean canTerminate) {
            this(
                visible, cause, luckRemaining, finalWords, countdownUntilMs, canReincarnate, canTerminate,
                "", 0, "", 0.0, 0, 0.0, 0, 0.0, false
            );
        }

        public static final State HIDDEN = new State(
            false, "", 0f, List.of(), 0L, false, false, "", 0, "", 0.0, 0, 0.0, 0, 0.0, false
        );

        public boolean hasLifespanPreview() { return lifespanCapByRealm > 0; }

        public long remainingMs(long nowMs) {
            return Math.max(0L, countdownUntilMs - nowMs);
        }
    }

    private static volatile State snapshot = State.HIDDEN;

    private DeathStateStore() {}

    public static State snapshot() { return snapshot; }

    public static void replace(State next) {
        snapshot = next == null ? State.HIDDEN : Objects.requireNonNull(next);
    }

    public static void hide() {
        snapshot = State.HIDDEN;
    }

    public static void resetForTests() {
        snapshot = State.HIDDEN;
    }
}
