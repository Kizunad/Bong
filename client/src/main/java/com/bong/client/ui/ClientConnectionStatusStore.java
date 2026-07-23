package com.bong.client.ui;

import com.bong.client.hud.BongToast;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.PlayerListEntry;

import java.util.IdentityHashMap;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;

public final class ClientConnectionStatusStore {
    private static final Object LOCK = new Object();
    /**
     * Fabric 为每条物理 PLAY 连接创建独立 ClientPlayNetworkHandler。这里按 identity
     * 保存 INIT 时分配的不可变 token；禁止用 equals() 把两个 handler 合并成一条 session。
     */
    private static final Map<Object, SessionToken> SESSION_TOKENS = new IdentityHashMap<>();

    private static volatile boolean observed;
    private static volatile boolean connected;
    private static volatile long connectedAtMs;
    private static volatile long lastPayloadAtMs;
    private static volatile long disconnectedAtMs;
    private static long nextSessionSequence;
    /**
     * 最近一次 INIT 新建的 token。只有它可以首次 JOIN；即使较旧 handler 的 JOIN 回调迟到，
     * 也不得重新夺回已经推进到更新物理连接的全局 session。
     */
    private static SessionToken newestSessionToken;
    private static SessionToken activeSessionToken;
    private static volatile ConnectionStatusIndicator.Status lastStatus = ConnectionStatusIndicator.Status.HIDDEN;

    private ClientConnectionStatusStore() {
    }

    /**
     * 每个物理 ClientPlayNetworkHandler 的不可变 session token。
     * 构造器私有，只有 {@link #initializeSession(Object)} 能分配，调用方无法按序号伪造 token。
     */
    public static final class SessionToken {
        private final long sequence;

        private SessionToken(long sequence) {
            this.sequence = sequence;
        }

        @Override
        public String toString() {
            return "SessionToken[" + sequence + "]";
        }
    }

    /**
     * Fabric {@code ClientPlayConnectionEvents.INIT} 入口。
     *
     * <p>INIT 在 ClientPlayNetworkHandler 构造末尾、任何 PLAY payload 可达前同步触发。
     * 同一 handler 的重复 INIT 幂等返回原 token，绝不为同一物理连接换代。</p>
     */
    public static SessionToken initializeSession(Object handler) {
        Objects.requireNonNull(handler, "handler");
        synchronized (LOCK) {
            SessionToken existing = SESSION_TOKENS.get(handler);
            if (existing != null) {
                return existing;
            }
            final long sequence;
            try {
                sequence = Math.incrementExact(nextSessionSequence);
            } catch (ArithmeticException overflow) {
                throw new IllegalStateException("Client connection session token sequence exhausted", overflow);
            }
            SessionToken token = new SessionToken(sequence);
            SESSION_TOKENS.put(handler, token);
            newestSessionToken = token;
            return token;
        }
    }

    /** raw receiver 必须按 Fabric 传入的 handler 捕获 token；未注册 handler 返回 empty。 */
    public static Optional<SessionToken> sessionToken(Object handler) {
        if (handler == null) {
            return Optional.empty();
        }
        synchronized (LOCK) {
            return Optional.ofNullable(SESSION_TOKENS.get(handler));
        }
    }

    /**
     * Fabric {@code JOIN} 入口：只激活 INIT 已分配的 token，不分配、不换代。
     * 未注册 handler fail closed，返回 false 且不改变连接状态。
     */
    public static boolean activateSession(Object handler, long nowMs) {
        SessionToken token;
        synchronized (LOCK) {
            token = SESSION_TOKENS.get(handler);
        }
        return activateSession(token, nowMs);
    }

    /**
     * Fabric JOIN callback 在 INIT 时已经捕获对应 token；用 token 而不是重新按 handler 查询，
     * 防止旧 handler 的迟到 JOIN 在新的物理连接已经 INIT/JOIN 后重新夺回 active session。
     */
    public static boolean activateSession(SessionToken token, long nowMs) {
        if (token == null) {
            return false;
        }
        synchronized (LOCK) {
            // token 必须仍在注册表中；DISCONNECT 已移除的 token 永远不能复活。
            if (!SESSION_TOKENS.containsValue(token)) {
                return false;
            }
            // INIT 已经观察到更新的物理 handler 后，较旧 handler 的迟到 JOIN 必须 fail closed。
            // 同一 active handler 的重复 JOIN 仍可进入下方幂等分支，保持 freshness 单调。
            if (activeSessionToken != token && newestSessionToken != token) {
                return false;
            }

            long now = Math.max(0L, nowMs);
            if (activeSessionToken != token) {
                activeSessionToken = token;
                connectedAtMs = now;
                lastPayloadAtMs = now;
            } else {
                // JOIN 回调意外重复时保持同一 session 的 freshness 单调，不重置历史。
                if (connectedAtMs == 0L) {
                    connectedAtMs = now;
                }
                lastPayloadAtMs = Math.max(lastPayloadAtMs, now);
            }
            observed = true;
            connected = true;
            disconnectedAtMs = 0L;
            return true;
        }
    }

    /**
     * Fabric {@code DISCONNECT} 入口：同步移除 handler→token 映射，使已排队 task 立即失效。
     *
     * @return true 仅当该 handler 正是当前 active session；调用方只有此时才应排全局 store 清理。
     *         INIT 后 JOIN 前断线、重复断线、旧 handler 的迟到断线都返回 false，避免误清新 session。
     */
    public static boolean invalidateSession(Object handler, long nowMs) {
        if (handler == null) {
            return false;
        }
        synchronized (LOCK) {
            SessionToken token = SESSION_TOKENS.remove(handler);
            if (token == null) {
                return false;
            }
            if (activeSessionToken != token) {
                return false;
            }

            activeSessionToken = null;
            observed = true;
            connected = false;
            disconnectedAtMs = Math.max(0L, nowMs);
            return true;
        }
    }

    /** queued payload task 只有在捕获 token 仍是当前已激活 session 时才可执行。 */
    public static boolean isActiveSession(SessionToken token) {
        if (token == null) {
            return false;
        }
        synchronized (LOCK) {
            return connected && activeSessionToken == token;
        }
    }

    /**
     * 以当前已激活 session 标记载荷到达（收包时刻语义）。
     * 其他 channel 仍可在 network thread 调用；INIT 后 JOIN 前或 DISCONNECT 后均 fail closed。
     */
    public static void markPayloadReceived(long nowMs) {
        synchronized (LOCK) {
            if (!connected || activeSessionToken == null) {
                return;
            }
            markPayloadReceivedLocked(nowMs);
        }
    }

    /**
     * 以 raw receiver 捕获的不可变 token + receivedAt 标记载荷。
     * token 已失效或尚未 JOIN 激活时整段 no-op；同一 token 内乱序、0/负时间戳不得回退 freshness。
     */
    public static void markPayloadReceived(long nowMs, SessionToken token) {
        synchronized (LOCK) {
            if (!connected || token == null || activeSessionToken != token) {
                return;
            }
            markPayloadReceivedLocked(nowMs);
        }
    }

    private static void markPayloadReceivedLocked(long nowMs) {
        long now = Math.max(0L, nowMs);
        observed = true;
        connected = true;
        if (connectedAtMs == 0L) {
            connectedAtMs = now;
        }
        lastPayloadAtMs = Math.max(lastPayloadAtMs, now);
        disconnectedAtMs = 0L;
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
            SESSION_TOKENS.clear();
            observed = false;
            connected = false;
            connectedAtMs = 0L;
            lastPayloadAtMs = 0L;
            disconnectedAtMs = 0L;
            nextSessionSequence = 0L;
            newestSessionToken = null;
            activeSessionToken = null;
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
