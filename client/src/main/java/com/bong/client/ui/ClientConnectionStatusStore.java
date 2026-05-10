package com.bong.client.ui;

import com.bong.client.hud.BongToast;

public final class ClientConnectionStatusStore {
    private static volatile boolean observed;
    private static volatile boolean connected;
    private static volatile long connectedAtMs;
    private static volatile long lastPayloadAtMs;
    private static volatile long disconnectedAtMs;
    private static volatile ConnectionStatusIndicator.Status lastStatus = ConnectionStatusIndicator.Status.HIDDEN;

    private ClientConnectionStatusStore() {
    }

    public static void markConnected(long nowMs) {
        long now = Math.max(0L, nowMs);
        observed = true;
        connected = true;
        connectedAtMs = now;
        lastPayloadAtMs = now;
        disconnectedAtMs = 0L;
    }

    public static void markPayloadReceived(long nowMs) {
        long now = Math.max(0L, nowMs);
        observed = true;
        connected = true;
        if (connectedAtMs == 0L) {
            connectedAtMs = now;
        }
        lastPayloadAtMs = now;
        disconnectedAtMs = 0L;
    }

    public static void markDisconnected(long nowMs) {
        observed = true;
        connected = false;
        disconnectedAtMs = Math.max(0L, nowMs);
    }

    public static ConnectionStatusIndicator.Snapshot snapshot(long nowMs) {
        if (!observed) {
            return ConnectionStatusIndicator.Snapshot.hidden();
        }
        long now = Math.max(0L, nowMs);
        long lastAge = lastPayloadAtMs == 0L ? Long.MAX_VALUE : Math.max(0L, now - lastPayloadAtMs);
        long disconnectedDuration = connected ? 0L : Math.max(0L, now - disconnectedAtMs);
        return ConnectionStatusIndicator.evaluate(connected, 42L, disconnectedDuration, lastAge);
    }

    public static void tick(long nowMs) {
        ConnectionStatusIndicator.Snapshot snapshot = snapshot(nowMs);
        ConnectionStatusIndicator.Status current = snapshot.status();
        ConnectionStatusIndicator.Status previous = lastStatus;
        if (current != previous) {
            if (current == ConnectionStatusIndicator.Status.RED) {
                BongToast.show("与天道失联", 0xFFFFAA55, nowMs, 3_000L);
            } else if (current == ConnectionStatusIndicator.Status.GREEN && previous == ConnectionStatusIndicator.Status.RED) {
                BongToast.show("天道重注", 0xFFAAFFAA, nowMs, 3_000L);
            }
            lastStatus = current;
        }
    }

    public static void resetForTests() {
        observed = false;
        connected = false;
        connectedAtMs = 0L;
        lastPayloadAtMs = 0L;
        disconnectedAtMs = 0L;
        lastStatus = ConnectionStatusIndicator.Status.HIDDEN;
    }
}
