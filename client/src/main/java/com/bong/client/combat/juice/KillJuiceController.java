package com.bong.client.combat.juice;

public final class KillJuiceController {
    private static final long MULTI_KILL_WINDOW_MS = 5_000L;
    private static final Object LOCK = new Object();
    private static KillState activeKill = KillState.none();
    private static MultiKillState multiKill = MultiKillState.empty();

    private KillJuiceController() {
    }

    public static KillState trigger(CombatJuiceEvent event, CombatJuiceProfile profile, long nowMs) {
        if (event == null || profile == null) {
            return KillState.none();
        }
        boolean localOnly = !event.localPlayerUuid().isBlank() && event.localPlayerIsAttacker();
        if (!localOnly) {
            return KillState.none();
        }
        KillState state = new KillState(
            event.attackerUuid(),
            event.victimName().isBlank() ? "target" : event.victimName(),
            nowMs,
            profile.killSlowmoFactor(),
            profile.killSlowmoTicks(),
            -5.0,
            event.rareDrop()
        );
        synchronized (LOCK) {
            activeKill = state;
            multiKill = nextMultiKillLocked(nowMs);
        }
        return state;
    }

    public static KillState activeKill(long nowMs) {
        synchronized (LOCK) {
            KillState state = activeKill;
            if (state == null || !state.activeAt(nowMs)) {
                activeKill = KillState.none();
                return KillState.none();
            }
            return state;
        }
    }

    public static double fovDelta(long nowMs) {
        KillState state = activeKill(nowMs);
        if (!state.activeAt(nowMs)) {
            return 0.0;
        }
        return state.fovDeltaDegrees() * state.remainingRatioAt(nowMs);
    }

    public static MultiKillState multiKill() {
        synchronized (LOCK) {
            return multiKill;
        }
    }

    public static void resetForTests() {
        synchronized (LOCK) {
            activeKill = KillState.none();
            multiKill = MultiKillState.empty();
        }
    }

    private static MultiKillState nextMultiKillLocked(long nowMs) {
        MultiKillState previous = multiKill;
        int count = previous != null && nowMs - previous.lastKillAtMs() <= MULTI_KILL_WINDOW_MS
            ? previous.count() + 1
            : 1;
        double shakeMultiplier = Math.min(2.0, 1.0 + Math.max(0, count - 1) * 0.2);
        double pitchMultiplier = Math.pow(1.1, Math.max(0, count - 1));
        return new MultiKillState(count, shakeMultiplier, pitchMultiplier, nowMs);
    }

    public record KillState(
        String killerUuid,
        String victimName,
        long startedAtMs,
        float slowmoFactor,
        int slowmoTicks,
        double fovDeltaDegrees,
        boolean rareDrop
    ) {
        public static KillState none() {
            return new KillState("", "", 0L, 1.0f, 0, 0.0, false);
        }

        public long durationMillis() {
            return Math.max(0, slowmoTicks) * 50L;
        }

        public boolean activeAt(long nowMs) {
            return !killerUuid.isBlank() && durationMillis() > 0L && nowMs - startedAtMs < durationMillis();
        }

        public double remainingRatioAt(long nowMs) {
            long duration = durationMillis();
            if (duration <= 0L) {
                return 0.0;
            }
            long elapsed = Math.max(0L, nowMs - startedAtMs);
            if (elapsed >= duration) {
                return 0.0;
            }
            return 1.0 - elapsed / (double) duration;
        }
    }

    public record MultiKillState(int count, double shakeMultiplier, double pitchMultiplier, long lastKillAtMs) {
        public static MultiKillState empty() {
            return new MultiKillState(0, 1.0, 1.0, 0L);
        }
    }
}
