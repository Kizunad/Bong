package com.bong.client.ui;

import com.bong.client.hud.BongToast;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.PlayerListEntry;

public final class ClientConnectionStatusStore {
    private static final Object LOCK = new Object();
    private static volatile boolean observed;
    private static volatile boolean connected;
    private static volatile long connectedAtMs;
    private static volatile long lastPayloadAtMs;
    private static volatile long disconnectedAtMs;
    private static volatile ConnectionStatusIndicator.Status lastStatus = ConnectionStatusIndicator.Status.HIDDEN;

    private ClientConnectionStatusStore() {
    }

    public static void markConnected(long nowMs) {
        synchronized (LOCK) {
            long now = Math.max(0L, nowMs);
            observed = true;
            connected = true;
            connectedAtMs = now;
            lastPayloadAtMs = now;
            disconnectedAtMs = 0L;
        }
    }

    public static void markPayloadReceived(long nowMs) {
        synchronized (LOCK) {
            long now = Math.max(0L, nowMs);
            observed = true;
            connected = true;
            if (connectedAtMs == 0L) {
                connectedAtMs = now;
            }
            lastPayloadAtMs = now;
            disconnectedAtMs = 0L;
        }
    }

    public static void markDisconnected(long nowMs) {
        synchronized (LOCK) {
            observed = true;
            connected = false;
            disconnectedAtMs = Math.max(0L, nowMs);
        }
    }

    public static ConnectionStatusIndicator.Snapshot snapshot(long nowMs) {
        synchronized (LOCK) {
            if (!observed) {
                return ConnectionStatusIndicator.Snapshot.hidden();
            }
            long now = Math.max(0L, nowMs);
            long lastAge = lastPayloadAtMs == 0L ? Long.MAX_VALUE : Math.max(0L, now - lastPayloadAtMs);
            long disconnectedDuration = connected ? 0L : Math.max(0L, now - disconnectedAtMs);
            return ConnectionStatusIndicator.evaluate(connected, currentNetworkLatencyMs(), disconnectedDuration, lastAge);
        }
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
        synchronized (LOCK) {
            observed = false;
            connected = false;
            connectedAtMs = 0L;
            lastPayloadAtMs = 0L;
            disconnectedAtMs = 0L;
            lastStatus = ConnectionStatusIndicator.Status.HIDDEN;
        }
    }

    private static long currentNetworkLatencyMs() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null || client.player == null || client.getNetworkHandler() == null) {
            return ConnectionStatusIndicator.UNKNOWN_LATENCY_MS;
        }
        PlayerListEntry entry = client.getNetworkHandler().getPlayerListEntry(client.player.getUuid());
        return entry == null ? ConnectionStatusIndicator.UNKNOWN_LATENCY_MS : Math.max(0, entry.getLatency());
    }
}
