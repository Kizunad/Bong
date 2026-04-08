package com.bong.client;

public final class PlayerStateCache {
    private static PlayerStateSnapshot snapshot;

    private PlayerStateCache() {
    }

    public static synchronized void update(
        String realm,
        double spiritQi,
        double karma,
        double compositePower,
        PowerBreakdown breakdown,
        String zone
    ) {
        update(new PlayerStateSnapshot(realm, spiritQi, karma, compositePower, breakdown, zone));
    }

    public static synchronized void update(PlayerStateSnapshot nextSnapshot) {
        if (!isValid(nextSnapshot)) {
            return;
        }

        snapshot = new PlayerStateSnapshot(
            nextSnapshot.realm().trim(),
            nextSnapshot.spiritQi(),
            nextSnapshot.karma(),
            nextSnapshot.compositePower(),
            nextSnapshot.breakdown(),
            nextSnapshot.zone().trim()
        );
    }

    public static synchronized PlayerStateSnapshot peek() {
        return snapshot;
    }

    static synchronized void clear() {
        snapshot = null;
    }

    private static boolean isValid(PlayerStateSnapshot nextSnapshot) {
        if (nextSnapshot == null) {
            return false;
        }
        if (nextSnapshot.realm() == null || nextSnapshot.realm().isBlank()) {
            return false;
        }
        if (!Double.isFinite(nextSnapshot.spiritQi()) || nextSnapshot.spiritQi() < 0.0) {
            return false;
        }
        if (!Double.isFinite(nextSnapshot.karma()) || nextSnapshot.karma() < -1.0 || nextSnapshot.karma() > 1.0) {
            return false;
        }
        if (!Double.isFinite(nextSnapshot.compositePower())
            || nextSnapshot.compositePower() < 0.0
            || nextSnapshot.compositePower() > 1.0) {
            return false;
        }
        if (nextSnapshot.zone() == null || nextSnapshot.zone().isBlank()) {
            return false;
        }

        return isValidBreakdown(nextSnapshot.breakdown());
    }

    private static boolean isValidBreakdown(PowerBreakdown breakdown) {
        if (breakdown == null) {
            return false;
        }

        return isUnitValue(breakdown.combat())
            && isUnitValue(breakdown.wealth())
            && isUnitValue(breakdown.social())
            && isUnitValue(breakdown.karma())
            && isUnitValue(breakdown.territory());
    }

    private static boolean isUnitValue(double value) {
        return Double.isFinite(value) && value >= 0.0 && value <= 1.0;
    }

    public record PlayerStateSnapshot(
        String realm,
        double spiritQi,
        double karma,
        double compositePower,
        PowerBreakdown breakdown,
        String zone
    ) {
    }

    public record PowerBreakdown(double combat, double wealth, double social, double karma, double territory) {
    }
}
