package com.bong.client.combat;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

/**
 * Volatile cast-state store (§11.1). Authoritative data comes from the server
 * via {@code bong:combat/cast_sync}; this mirror is read by the HUD planner and
 * by local predicted-interrupt checks.
 */
public final class CastStateStore {
    /** Default short cooldown after an interrupt (§4.4). */
    public static final long SHORT_INTERRUPT_COOLDOWN_MS = 500L;

    private static volatile CastState snapshot = CastState.idle();
    private static final List<Consumer<CastState>> listeners = new CopyOnWriteArrayList<>();

    private CastStateStore() {
    }

    public static CastState snapshot() {
        return snapshot;
    }

    public static void replace(CastState next) {
        setSnapshot(next == null ? CastState.idle() : next);
    }

    public static void addListener(Consumer<CastState> listener) {
        if (listener != null) listeners.add(listener);
    }

    public static void removeListener(Consumer<CastState> listener) {
        listeners.remove(listener);
    }

    /** Begin casting (Idle → Casting). No-op if already casting. */
    public static void beginCast(int slot, int durationMs, long startedAtMs) {
        beginCast(CastState.Source.QUICK_SLOT, slot, durationMs, startedAtMs);
    }

    public static void beginSkillBarCast(int slot, int durationMs, long startedAtMs) {
        beginCast(CastState.Source.SKILL_BAR, slot, durationMs, startedAtMs);
    }

    public static void beginCast(CastState.Source source, int slot, int durationMs, long startedAtMs) {
        CastState current = snapshot;
        if (current.isCasting()) {
            return;
        }
        setSnapshot(CastState.casting(source, slot, durationMs, startedAtMs));
    }

    /** Casting → Complete when duration has elapsed. */
    public static void complete(long nowMs) {
        CastState current = snapshot;
        if (!current.isCasting()) return;
        setSnapshot(current.transitionToComplete(nowMs));
    }

    /** Casting → Interrupt with a reason. Idempotent (stays in interrupt state). */
    public static void interrupt(CastOutcome reason, long nowMs) {
        CastState current = snapshot;
        if (current.phase() == CastState.Phase.IDLE) return;
        if (current.phase() == CastState.Phase.INTERRUPT) return;
        setSnapshot(current.transitionToInterrupt(reason, nowMs));
    }

    /**
     * After the 0.3s cast-bar fade (§4.1), revert to idle. Safe to call every
     * frame.
     */
    public static void tick(long nowMs) {
        CastState current = snapshot;
        if (current.phase() == CastState.Phase.CASTING) {
            if (current.durationMs() > 0
                && nowMs - current.startedAtMs() >= current.durationMs()) {
                setSnapshot(current.transitionToComplete(nowMs));
            }
            return;
        }
        if (current.phase() == CastState.Phase.COMPLETE
            || current.phase() == CastState.Phase.INTERRUPT) {
            if (nowMs - current.endedAtMs() >= 300L) {
                setSnapshot(CastState.idle());
            }
        }
    }

    public static void resetForTests() {
        snapshot = CastState.idle();
        listeners.clear();
    }

    private static void setSnapshot(CastState next) {
        snapshot = next == null ? CastState.idle() : next;
        for (Consumer<CastState> listener : listeners) {
            listener.accept(snapshot);
        }
    }
}
