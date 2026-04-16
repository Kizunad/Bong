package com.bong.client.combat;

public final class DefenseWindowStore {
    private static volatile DefenseWindowState snapshot = DefenseWindowState.idle();

    private DefenseWindowStore() {
    }

    public static DefenseWindowState snapshot() {
        return snapshot;
    }

    public static void open(int durationMs, long nowMs) {
        snapshot = DefenseWindowState.active(durationMs, nowMs, nowMs + Math.max(0, durationMs));
    }

    /** Replace with a server-provided snapshot (preserves authoritative timestamps). */
    public static void replaceSnapshot(DefenseWindowState next) {
        snapshot = next == null ? DefenseWindowState.idle() : next;
    }

    public static void close() {
        snapshot = DefenseWindowState.idle();
    }

    public static void tick(long nowMs) {
        DefenseWindowState s = snapshot;
        if (s.active() && s.isExpired(nowMs)) {
            snapshot = DefenseWindowState.idle();
        }
    }

    public static void resetForTests() {
        snapshot = DefenseWindowState.idle();
    }
}
