package com.bong.client.movement;

import java.util.Objects;

public final class MovementStateStore {
    private static volatile MovementState snapshot = MovementState.empty();

    private MovementStateStore() {
    }

    public static synchronized MovementState snapshot() {
        return snapshot;
    }

    public static synchronized void replace(MovementState next, long nowMs) {
        MovementState current = snapshot;
        MovementState normalized = next == null ? MovementState.empty() : next;
        long hudActivityAtMs = current.hudActivityAtMs();
        long rejectedAtMs = current.rejectedAtMs();

        boolean rejected = !normalized.rejectedAction().isEmpty();
        boolean newActionTick = normalized.lastActionTick() != null
            && !Objects.equals(normalized.lastActionTick(), current.lastActionTick());
        boolean actionStarted = current.action() == MovementState.Action.NONE
            && normalized.action() != MovementState.Action.NONE;
        boolean cooldownFinished = current.dashCooldownRemainingTicks() > 0
            && normalized.dashCooldownRemainingTicks() == 0;

        if (rejected) {
            hudActivityAtMs = nowMs;
            rejectedAtMs = nowMs;
        } else if (newActionTick || actionStarted || cooldownFinished) {
            hudActivityAtMs = nowMs;
        }

        snapshot = normalized.withTiming(nowMs, hudActivityAtMs, rejectedAtMs);
    }

    public static synchronized void clear() {
        snapshot = MovementState.empty();
    }


    public static synchronized void clearOnDisconnect() {
        clear();
    }

    public static void resetForTests() {
        clear();
    }
}
