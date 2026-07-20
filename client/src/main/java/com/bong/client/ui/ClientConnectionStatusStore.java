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
    /**
     * 连接代次：每次 {@link #markConnected(long)} / {@link #markDisconnected(long)} 递增。
     * 收包时刻捕获的 generation 与当前不一致时，禁止 stale task 复活 connected
     * 或刷新 lastPayloadAtMs。
     */
    private static volatile long connectionGeneration;
    private static volatile ConnectionStatusIndicator.Status lastStatus = ConnectionStatusIndicator.Status.HIDDEN;

    private ClientConnectionStatusStore() {
    }

    /** 当前连接代次（线程安全）。receiver 应在收包瞬间捕获并随 task 传递。 */
    public static long currentGeneration() {
        synchronized (LOCK) {
            return connectionGeneration;
        }
    }

    /** stale task / 旧连接 payload 判定：generation 必须仍是当前代次。 */
    public static boolean isCurrentGeneration(long generation) {
        synchronized (LOCK) {
            return generation == connectionGeneration;
        }
    }

    public static void markConnected(long nowMs) {
        synchronized (LOCK) {
            long now = Math.max(0L, nowMs);
            connectionGeneration++;
            observed = true;
            connected = true;
            connectedAtMs = now;
            lastPayloadAtMs = now;
            disconnectedAtMs = 0L;
        }
    }

    /**
     * 以<strong>当前</strong>连接代次标记载荷到达（收包时刻语义）。
     * 其他 channel 仍可在 network thread 直接调用；store 内部 synchronized。
     */
    public static void markPayloadReceived(long nowMs) {
        long generation;
        synchronized (LOCK) {
            generation = connectionGeneration;
        }
        markPayloadReceived(nowMs, generation);
    }

    /**
     * 以收包瞬间捕获的 generation + receivedAt 标记载荷。
     * generation 与当前代次不一致时整段 no-op，防止 disconnect-before-drain /
     * reconnect 后的 stale task 把状态机复活为 connected 或用 processing time 污染 freshness。
     * 同一代次的跨 channel / queue 乱序不得让 lastPayloadAtMs 回退。
     */
    public static void markPayloadReceived(long nowMs, long generation) {
        synchronized (LOCK) {
            if (generation != connectionGeneration) {
                return;
            }
            long now = Math.max(0L, nowMs);
            observed = true;
            connected = true;
            if (connectedAtMs == 0L) {
                connectedAtMs = now;
            }
            lastPayloadAtMs = Math.max(lastPayloadAtMs, now);
            disconnectedAtMs = 0L;
        }
    }

    public static void markDisconnected(long nowMs) {
        synchronized (LOCK) {
            connectionGeneration++;
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

    /** 测试观察点：最近一次成功标记的载荷时间（未观察时为 0）。 */
    public static long lastPayloadAtMsForTests() {
        synchronized (LOCK) {
            return lastPayloadAtMs;
        }
    }

    /** 测试观察点：当前是否视为已连接。 */
    public static boolean connectedForTests() {
        synchronized (LOCK) {
            return connected;
        }
    }

    public static void tick(long nowMs) {
        tick(nowMs, nowMs);
    }

    public static void tick(long nowMs, long toastNowMs) {
        ConnectionStatusIndicator.Snapshot snapshot = snapshot(nowMs);
        ConnectionStatusIndicator.Status current = snapshot.status();
        ConnectionStatusIndicator.Status previous = lastStatus;
        if (current != previous) {
            if (current == ConnectionStatusIndicator.Status.RED) {
                BongToast.show("与天道失联", 0xFFFFAA55, toastNowMs, 3_000L);
            } else if (current == ConnectionStatusIndicator.Status.GREEN && previous == ConnectionStatusIndicator.Status.RED) {
                BongToast.show("天道重注", 0xFFAAFFAA, toastNowMs, 3_000L);
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
            connectionGeneration = 0L;
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
