package com.bong.client;

import java.util.Objects;

public final class PlayerStateState {
    static final double MIN_KARMA = -1.0d;
    static final double MAX_KARMA = 1.0d;
    static final double MIN_UNIT_VALUE = 0.0d;
    static final double MAX_UNIT_VALUE = 1.0d;

    private static PlayerStateSnapshot currentPlayerState;

    private PlayerStateState() {
    }

    static PlayerStateSnapshot record(BongServerPayload.PlayerState playerState) {
        return record(playerState, System.currentTimeMillis());
    }

    static PlayerStateSnapshot record(BongServerPayload.PlayerState playerState, long nowMs) {
        Objects.requireNonNull(playerState, "playerState");

        PlayerStateSnapshot snapshot = snapshotOf(playerState, nowMs);
        currentPlayerState = snapshot;
        return snapshot;
    }

    static PlayerStateSnapshot snapshotOf(BongServerPayload.PlayerState playerState, long nowMs) {
        Objects.requireNonNull(playerState, "playerState");

        return new PlayerStateSnapshot(
                normalizeRealmKey(playerState.realm()),
                clampSpiritQi(playerState.spiritQi(), normalizeSpiritQiMax(playerState.spiritQiMax())),
                normalizeSpiritQiMax(playerState.spiritQiMax()),
                clampKarma(playerState.karma()),
                clampUnit(playerState.compositePower()),
                normalizeZoneKey(playerState.zone()),
                nowMs
        );
    }

    static String normalizeRealmKey(String realm) {
        if (realm == null) {
            return "";
        }

        String normalized = realm.trim();
        return normalized.isEmpty() ? "" : normalized;
    }

    static String normalizeZoneKey(String zone) {
        if (zone == null) {
            return "unknown_zone";
        }

        String normalized = zone.trim();
        return normalized.isEmpty() ? "unknown_zone" : normalized;
    }

    static double normalizeSpiritQiMax(double spiritQiMax) {
        if (!Double.isFinite(spiritQiMax) || spiritQiMax <= 0.0d) {
            return 1.0d;
        }

        return spiritQiMax;
    }

    static double clampSpiritQi(double spiritQi, double spiritQiMax) {
        if (!Double.isFinite(spiritQi)) {
            return 0.0d;
        }

        return Math.max(0.0d, Math.min(spiritQiMax, spiritQi));
    }

    static double spiritQiRatio(double spiritQi, double spiritQiMax) {
        if (spiritQiMax <= 0.0d) {
            return 0.0d;
        }

        return clampUnit(spiritQi / spiritQiMax);
    }

    static double clampKarma(double karma) {
        if (!Double.isFinite(karma)) {
            return 0.0d;
        }

        return Math.max(MIN_KARMA, Math.min(MAX_KARMA, karma));
    }

    static double clampUnit(double value) {
        if (!Double.isFinite(value)) {
            return 0.0d;
        }

        return Math.max(MIN_UNIT_VALUE, Math.min(MAX_UNIT_VALUE, value));
    }

    public static PlayerStateSnapshot getCurrentPlayerState() {
        return currentPlayerState;
    }

    public static void clear() {
        currentPlayerState = null;
    }

    public record PlayerStateSnapshot(
            String realmKey,
            double spiritQi,
            double spiritQiMax,
            double karma,
            double compositePower,
            String zoneKey,
            long updatedAtMs
    ) {
        public PlayerStateSnapshot {
            Objects.requireNonNull(realmKey, "realmKey");
            Objects.requireNonNull(zoneKey, "zoneKey");
        }
    }
}
