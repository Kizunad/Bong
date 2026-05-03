package com.bong.client.combat;

/**
 * Server-pushed Jiemai (截脉) prep window (§3.2 / §11.4).
 */
public final class DefenseWindowState {
    private static final DefenseWindowState IDLE = new DefenseWindowState(false, 0, 0L, 0L);

    private final boolean active;
    private final int durationMs;
    private final long startedAtMs;
    private final long expiresAtMs;

    private DefenseWindowState(boolean active, int durationMs, long startedAtMs, long expiresAtMs) {
        this.active = active;
        this.durationMs = durationMs;
        this.startedAtMs = startedAtMs;
        this.expiresAtMs = expiresAtMs;
    }

    public static DefenseWindowState idle() {
        return IDLE;
    }

    public static DefenseWindowState active(int durationMs, long startedAtMs, long expiresAtMs) {
        return new DefenseWindowState(true, Math.max(0, durationMs), startedAtMs, expiresAtMs);
    }

    public boolean active() { return active; }
    public int durationMs() { return durationMs; }
    public long startedAtMs() { return startedAtMs; }
    public long expiresAtMs() { return expiresAtMs; }

    /**
     * @return progress through the window in [0,1], where 1.0 is about to
     *         expire. Only valid when {@link #active()}.
     */
    public float progress(long nowMs) {
        if (!active || durationMs <= 0) return 0.0f;
        long elapsed = nowMs - startedAtMs;
        if (elapsed <= 0L) return 0.0f;
        if (elapsed >= durationMs) return 1.0f;
        return (float) elapsed / (float) durationMs;
    }

    public boolean isExpired(long nowMs) {
        return !active || nowMs >= expiresAtMs;
    }
}
