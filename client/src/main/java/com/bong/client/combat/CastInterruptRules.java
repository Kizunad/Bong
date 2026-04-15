package com.bong.client.combat;

/**
 * Pure functions implementing the cast interrupt matrix (§4.3) — kept separate
 * from the store so they're trivially unit-testable without any Minecraft
 * singletons.
 */
public final class CastInterruptRules {
    public static final double MOVEMENT_THRESHOLD_METERS = 0.3;
    /** Per-second contam threshold expressed as a fraction of maxHp (§4.3). */
    public static final double CONTAM_RATE_PER_MAXHP_PER_SECOND = 0.05;

    private CastInterruptRules() {
    }

    /**
     * @param cumulativeMovementMeters distance accumulated since cast began
     * @return true if movement exceeds the configured threshold
     */
    public static boolean movementInterrupts(double cumulativeMovementMeters) {
        return cumulativeMovementMeters > MOVEMENT_THRESHOLD_METERS;
    }

    /**
     * @param contamSinceCastStart cumulative contam damage since cast start
     * @param maxHp player's max HP (must be > 0)
     * @param castElapsedMs milliseconds elapsed since cast start
     * @return true if contam exceeds {@code duration * 0.05 * maxHp}
     */
    public static boolean contamInterrupts(double contamSinceCastStart, double maxHp, long castElapsedMs) {
        if (maxHp <= 0.0 || castElapsedMs <= 0L) return false;
        double seconds = castElapsedMs / 1000.0;
        double threshold = seconds * CONTAM_RATE_PER_MAXHP_PER_SECOND * maxHp;
        return contamSinceCastStart > threshold;
    }

    public enum ControlEffect { STUN, SILENCED_PHYSICAL, KNOCKBACK, CHARMED, SLOWED, DAMAGE_AMP, OTHER }

    /** Stun / Silenced / Knockback / Charmed all interrupt (§4.3). */
    public static boolean controlInterrupts(ControlEffect effect) {
        if (effect == null) return false;
        return switch (effect) {
            case STUN, SILENCED_PHYSICAL, KNOCKBACK, CHARMED -> true;
            default -> false;
        };
    }
}
