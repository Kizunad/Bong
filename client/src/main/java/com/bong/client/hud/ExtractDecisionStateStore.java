package com.bong.client.hud;

import java.util.concurrent.atomic.AtomicReference;

public final class ExtractDecisionStateStore {
    public static final int UNKNOWN_RISK_SCORE = -1;
    public static final long EMERGENCY_WINDOW_MS = 15_000L;

    private static final AtomicReference<Snapshot> STATE = new AtomicReference<>(Snapshot.EMPTY);

    private ExtractDecisionStateStore() {
    }

    public record Snapshot(
        long runStartedAtMs,
        int nearbyRiskScore,
        boolean realmMismatch,
        long emergencyUntilMs,
        int previousRiskScore
    ) {
        public static final Snapshot EMPTY = new Snapshot(0L, UNKNOWN_RISK_SCORE, false, 0L, UNKNOWN_RISK_SCORE);

        public Snapshot {
            runStartedAtMs = Math.max(0L, runStartedAtMs);
            nearbyRiskScore = normalizeRiskScore(nearbyRiskScore);
            emergencyUntilMs = Math.max(0L, emergencyUntilMs);
            previousRiskScore = normalizeRiskScore(previousRiskScore);
        }

        public boolean hasRunClock() {
            return runStartedAtMs > 0L;
        }

        public boolean hasRiskScore() {
            return nearbyRiskScore >= 0;
        }

        public boolean emergencyActive(long nowMillis) {
            return emergencyUntilMs > Math.max(0L, nowMillis);
        }

        public boolean active(long nowMillis) {
            return hasRunClock() || hasRiskScore() || emergencyActive(nowMillis);
        }
    }

    public static Snapshot snapshot() {
        return STATE.get();
    }

    public static void startRun(long nowMillis) {
        Snapshot current = STATE.get();
        STATE.set(new Snapshot(
            Math.max(0L, nowMillis),
            current.nearbyRiskScore(),
            current.realmMismatch(),
            current.emergencyUntilMs(),
            current.previousRiskScore()
        ));
    }

    public static void markPlayerInWorld(long nowMillis) {
        Snapshot current = STATE.get();
        if (current.hasRunClock()) {
            return;
        }
        startRun(nowMillis);
    }

    public static void endRun() {
        STATE.set(Snapshot.EMPTY);
    }

    public static void clearOnDisconnect() {
        endRun();
    }

    public static void replaceRiskScore(int riskScore, boolean realmMismatch, long nowMillis) {
        Snapshot current = STATE.get();
        int normalizedRisk = normalizeRiskScore(riskScore);
        boolean emergencySpike = realmMismatch
            && normalizedRisk >= 80
            && (current.nearbyRiskScore() < 60 || normalizedRisk - current.nearbyRiskScore() >= 20);
        long emergencyUntil = emergencySpike
            ? Math.max(0L, nowMillis) + EMERGENCY_WINDOW_MS
            : current.emergencyUntilMs();
        STATE.set(new Snapshot(
            current.runStartedAtMs(),
            normalizedRisk,
            realmMismatch,
            emergencyUntil,
            current.nearbyRiskScore()
        ));
    }

    static void replaceForTests(Snapshot snapshot) {
        STATE.set(snapshot == null ? Snapshot.EMPTY : snapshot);
    }

    public static void resetForTests() {
        STATE.set(Snapshot.EMPTY);
    }

    private static int normalizeRiskScore(int value) {
        if (value < 0) {
            return UNKNOWN_RISK_SCORE;
        }
        return Math.min(100, value);
    }
}
