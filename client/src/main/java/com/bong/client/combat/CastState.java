package com.bong.client.combat;

/**
 * Client snapshot of the quick-slot cast state machine (§4).
 *
 * <p>Four terminal phases: {@code Idle}, {@code Casting}, {@code Complete},
 * {@code Interrupt}. The store transitions between them in response to server
 * {@code CastSync} payloads plus local predicted interrupts (see
 * {@link CastStateStore}).
 */
public final class CastState {
    public enum Phase { IDLE, CASTING, COMPLETE, INTERRUPT }
    public enum Source { QUICK_SLOT, SKILL_BAR }

    private static final CastState IDLE = new CastState(
        Phase.IDLE, Source.QUICK_SLOT, -1, 0, 0L, CastOutcome.NONE, 0L);

    private final Phase phase;
    private final Source source;
    private final int slot;
    private final int durationMs;
    private final long startedAtMs;
    private final CastOutcome outcome;
    private final long endedAtMs;

    private CastState(
        Phase phase,
        Source source,
        int slot,
        int durationMs,
        long startedAtMs,
        CastOutcome outcome,
        long endedAtMs
    ) {
        this.phase = phase;
        this.source = source == null ? Source.QUICK_SLOT : source;
        this.slot = slot;
        this.durationMs = durationMs;
        this.startedAtMs = startedAtMs;
        this.outcome = outcome;
        this.endedAtMs = endedAtMs;
    }

    public static CastState idle() {
        return IDLE;
    }

    public static CastState casting(int slot, int durationMs, long startedAtMs) {
        return casting(Source.QUICK_SLOT, slot, durationMs, startedAtMs);
    }

    public static CastState casting(Source source, int slot, int durationMs, long startedAtMs) {
        return new CastState(Phase.CASTING, source, slot, Math.max(0, durationMs), startedAtMs, CastOutcome.NONE, 0L);
    }

    public CastState transitionToComplete(long endedAtMs) {
        return new CastState(Phase.COMPLETE, source, slot, durationMs, startedAtMs, CastOutcome.COMPLETED, endedAtMs);
    }

    public CastState transitionToInterrupt(CastOutcome reason, long endedAtMs) {
        CastOutcome effective = reason == null || reason == CastOutcome.NONE || reason == CastOutcome.COMPLETED
            ? CastOutcome.USER_CANCEL
            : reason;
        return new CastState(Phase.INTERRUPT, source, slot, durationMs, startedAtMs, effective, endedAtMs);
    }

    public Phase phase() {
        return phase;
    }

    public Source source() {
        return source;
    }

    public int slot() {
        return slot;
    }

    public int durationMs() {
        return durationMs;
    }

    public long startedAtMs() {
        return startedAtMs;
    }

    public CastOutcome outcome() {
        return outcome;
    }

    public long endedAtMs() {
        return endedAtMs;
    }

    public boolean isCasting() {
        return phase == Phase.CASTING;
    }

    public boolean isIdle() {
        return phase == Phase.IDLE;
    }

    /** Normalized cast progress [0,1]; only meaningful while {@link #isCasting()}. */
    public float progress(long nowMs) {
        if (phase != Phase.CASTING || durationMs <= 0) return 0.0f;
        long elapsed = nowMs - startedAtMs;
        if (elapsed <= 0L) return 0.0f;
        if (elapsed >= durationMs) return 1.0f;
        return (float) elapsed / (float) durationMs;
    }
}
