package com.bong.client.combat.store;

public final class FullPowerStateStore {
    private static final FullPowerStateStore INSTANCE = new FullPowerStateStore();

    private volatile ChargingState charging = ChargingState.inactive();
    private volatile ExhaustedState exhausted = ExhaustedState.inactive();
    private volatile ReleaseEvent lastRelease = ReleaseEvent.empty();

    private FullPowerStateStore() {}

    public static ChargingState charging() {
        return INSTANCE.charging;
    }

    public static ExhaustedState exhausted() {
        return INSTANCE.exhausted;
    }

    public static ReleaseEvent lastRelease() {
        return INSTANCE.lastRelease;
    }

    public static void updateCharging(ChargingState state) {
        INSTANCE.charging = state == null ? ChargingState.inactive() : state;
    }

    public static void clearCharging() {
        INSTANCE.charging = ChargingState.inactive();
    }

    public static void updateExhausted(ExhaustedState state) {
        INSTANCE.exhausted = state == null ? ExhaustedState.inactive() : state;
    }

    public static void clearExhausted() {
        INSTANCE.exhausted = ExhaustedState.inactive();
    }

    public static void recordRelease(ReleaseEvent event) {
        INSTANCE.lastRelease = event == null ? ReleaseEvent.empty() : event;
    }

    public static void resetForTests() {
        INSTANCE.charging = ChargingState.inactive();
        INSTANCE.exhausted = ExhaustedState.inactive();
        INSTANCE.lastRelease = ReleaseEvent.empty();
    }

    public record ChargingState(
        boolean active,
        String casterUuid,
        double qiCommitted,
        double targetQi,
        long startedTick,
        long updatedAtMs
    ) {
        public ChargingState {
            casterUuid = casterUuid == null ? "" : casterUuid;
            qiCommitted = sanitizeNonNegative(qiCommitted);
            targetQi = sanitizeNonNegative(targetQi);
            startedTick = Math.max(0L, startedTick);
            updatedAtMs = Math.max(0L, updatedAtMs);
        }

        public static ChargingState inactive() {
            return new ChargingState(false, "", 0.0, 0.0, 0L, 0L);
        }

        public double progress() {
            if (!active || targetQi <= 1e-6) {
                return 0.0;
            }
            return Math.max(0.0, Math.min(1.0, qiCommitted / targetQi));
        }
    }

    public record ExhaustedState(
        boolean active,
        String casterUuid,
        long startedTick,
        long recoveryAtTick,
        long updatedAtMs
    ) {
        public ExhaustedState {
            casterUuid = casterUuid == null ? "" : casterUuid;
            startedTick = Math.max(0L, startedTick);
            recoveryAtTick = Math.max(startedTick, recoveryAtTick);
            updatedAtMs = Math.max(0L, updatedAtMs);
        }

        public static ExhaustedState inactive() {
            return new ExhaustedState(false, "", 0L, 0L, 0L);
        }

        public long remainingTicks(long nowMs) {
            if (!active) {
                return 0L;
            }
            long elapsedTicks = Math.max(0L, nowMs - updatedAtMs) / 50L;
            long serverRemainingAtUpdate = recoveryAtTick - startedTick;
            return Math.max(0L, serverRemainingAtUpdate - elapsedTicks);
        }
    }

    public record ReleaseEvent(
        String casterUuid,
        String targetUuid,
        double qiReleased,
        long tick,
        long receivedAtMs
    ) {
        public ReleaseEvent {
            casterUuid = casterUuid == null ? "" : casterUuid;
            targetUuid = targetUuid == null ? "" : targetUuid;
            qiReleased = sanitizeNonNegative(qiReleased);
            tick = Math.max(0L, tick);
            receivedAtMs = Math.max(0L, receivedAtMs);
        }

        public static ReleaseEvent empty() {
            return new ReleaseEvent("", "", 0.0, 0L, 0L);
        }
    }

    private static double sanitizeNonNegative(double value) {
        if (!Double.isFinite(value) || value < 0.0) {
            return 0.0;
        }
        return value;
    }
}
