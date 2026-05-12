package com.bong.client.death;

import java.util.Collections;
import java.util.List;

public record DeathCinematicState(
    boolean active,
    String characterId,
    Phase phase,
    long phaseTick,
    long phaseDurationTicks,
    long totalElapsedTicks,
    long totalDurationTicks,
    Roll roll,
    List<String> insightText,
    boolean finalDeath,
    int deathNumber,
    String zoneKind,
    boolean tsyDeath,
    long rebirthWeakenedTicks,
    boolean skipPredeath,
    long receivedAtMillis
) {
    public static final DeathCinematicState INACTIVE = new DeathCinematicState(
        false, "", Phase.PREDEATH, 0L, 1L, 0L, 1L,
        new Roll(0.0, 0.0, 0.0, RollResult.PENDING),
        List.of(), false, 0, "", false, 0L, false, 0L
    );

    public DeathCinematicState {
        characterId = characterId == null ? "" : characterId;
        phase = phase == null ? Phase.PREDEATH : phase;
        phaseTick = Math.max(0L, phaseTick);
        phaseDurationTicks = Math.max(1L, phaseDurationTicks);
        totalElapsedTicks = Math.max(0L, totalElapsedTicks);
        totalDurationTicks = Math.max(1L, totalDurationTicks);
        roll = roll == null ? new Roll(0.0, 0.0, 0.0, RollResult.PENDING) : roll;
        insightText = insightText == null ? Collections.emptyList() : List.copyOf(insightText);
        deathNumber = Math.max(0, deathNumber);
        zoneKind = zoneKind == null ? "" : zoneKind;
        rebirthWeakenedTicks = Math.max(0L, rebirthWeakenedTicks);
        receivedAtMillis = Math.max(0L, receivedAtMillis);
    }

    public double phaseProgress() {
        return clamp01(phaseTick / (double) phaseDurationTicks);
    }

    public double totalProgress() {
        return clamp01(totalElapsedTicks / (double) totalDurationTicks);
    }

    public DeathCinematicState advancedTo(long nowMillis) {
        if (!active || receivedAtMillis <= 0L || nowMillis <= receivedAtMillis) return this;
        long elapsedClientTicks = (nowMillis - receivedAtMillis) / 50L;
        if (elapsedClientTicks <= 0L) return this;
        long advancedTotal = Math.min(totalDurationTicks, totalElapsedTicks + elapsedClientTicks);
        PhasePosition position = phasePosition(advancedTotal);
        return new DeathCinematicState(
            active,
            characterId,
            position.phase(),
            position.phaseTick(),
            position.phaseDurationTicks(),
            advancedTotal,
            totalDurationTicks,
            roll,
            insightText,
            finalDeath,
            deathNumber,
            zoneKind,
            tsyDeath,
            rebirthWeakenedTicks,
            skipPredeath,
            receivedAtMillis
        );
    }

    private PhasePosition phasePosition(long elapsedTicks) {
        long remaining = Math.max(0L, elapsedTicks);
        Phase[] phases = {
            Phase.PREDEATH,
            Phase.DEATH_MOMENT,
            Phase.ROLL,
            Phase.INSIGHT_OVERLAY,
            Phase.DARKNESS,
            Phase.REBIRTH
        };
        long[] durations = phaseDurations();
        for (int i = 0; i < phases.length; i++) {
            long duration = durations[i];
            if (duration <= 0L) continue;
            if (remaining < duration) {
                return new PhasePosition(phases[i], remaining, duration);
            }
            remaining -= duration;
        }
        return new PhasePosition(Phase.REBIRTH, Math.max(0L, durations[5]), Math.max(1L, durations[5]));
    }

    private long[] phaseDurations() {
        boolean shortened = deathNumber >= 2 && !finalDeath;
        return new long[] {
            skipPredeath ? 0L : 60L,
            skipPredeath ? 0L : 20L,
            shortened ? 40L : 80L,
            shortened ? 60L : 120L,
            40L,
            60L
        };
    }

    static double clamp01(double value) {
        if (!Double.isFinite(value)) return 0.0;
        return Math.max(0.0, Math.min(1.0, value));
    }

    public enum Phase {
        PREDEATH("predeath"),
        DEATH_MOMENT("death_moment"),
        ROLL("roll"),
        INSIGHT_OVERLAY("insight_overlay"),
        DARKNESS("darkness"),
        REBIRTH("rebirth");

        private final String wireName;

        Phase(String wireName) {
            this.wireName = wireName;
        }

        public String wireName() {
            return wireName;
        }

        public static Phase fromWire(String wireName) {
            for (Phase phase : values()) {
                if (phase.wireName.equals(wireName)) return phase;
            }
            return PREDEATH;
        }
    }

    public enum RollResult {
        PENDING("pending"),
        SURVIVE("survive"),
        FALL("fall"),
        FINAL("final");

        private final String wireName;

        RollResult(String wireName) {
            this.wireName = wireName;
        }

        public String wireName() {
            return wireName;
        }

        public static RollResult fromWire(String wireName) {
            for (RollResult result : values()) {
                if (result.wireName.equals(wireName)) return result;
            }
            return PENDING;
        }
    }

    public record Roll(double probability, double threshold, double luckValue, RollResult result) {
        public Roll {
            probability = clamp01(probability);
            threshold = clamp01(threshold);
            luckValue = clamp01(luckValue);
            result = result == null ? RollResult.PENDING : result;
        }
    }

    private record PhasePosition(Phase phase, long phaseTick, long phaseDurationTicks) {}
}
