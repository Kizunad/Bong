package com.bong.client.agentui;

import org.jetbrains.annotations.Nullable;

/**
 * plan-agent-ui-data-v1 P1 — 客户端当前活跃 AgentUiScreen 单例存储。
 *
 * <p>单面板互斥：同一时刻最多一个活跃 session。
 * 新请求覆盖旧 session（server close 信号先到时由 {@link #receiveClose} 清除）。
 */
public final class AgentUiStore {
    /** 本地响应等待 server 错误 close 的最长窗口。 */
    static final long PENDING_ERROR_CLOSE_TTL_MILLIS = 10_000L;
    static final long PENDING_ERROR_CLOSE_TTL_NANOS =
        PENDING_ERROR_CLOSE_TTL_MILLIS * 1_000_000L;

    private AgentUiStore() {}

    @Nullable
    private static volatile AgentUiScreen activeScreen = null;
    @Nullable
    private static volatile PendingErrorClose pendingErrorClose = null;

    /** 设置当前活跃 screen（替换旧的，旧的由调用方负责关闭）。 */
    public static void setActive(@Nullable AgentUiScreen screen) {
        activeScreen = screen;
        // 新面板（或显式生命周期重置）开始后，旧 request 的迟到错误不得污染新生命周期。
        pendingErrorClose = null;
    }

    /** 若给定 screen 仍是当前活跃面板，则清除；防止旧面板迟到关闭误清新面板。 */
    static void clearIfActive(AgentUiScreen screen) {
        if (activeScreen == screen) {
            activeScreen = null;
        }
    }

    /** 获取当前活跃 screen（null = 无活跃面板）。 */
    @Nullable
    public static AgentUiScreen getActive() {
        return activeScreen;
    }

    /**
     * 接收 server close 信号（{@code agent_ui_close}）。
     *
     * <p>若当前活跃 screen 的 request_id 匹配，则调用 receiveCloseSignal 并清除。
     *
     * @param requestId close 信号携带的 request_id
     * @param reason    关闭原因（null / "" = Replaced；非空 = 错误）
     */
    public static void receiveClose(String requestId, @Nullable String reason) {
        receiveCloseAtNanos(requestId, reason, System.nanoTime(), System.currentTimeMillis());
    }

    static void receiveCloseAtNanos(
        String requestId,
        @Nullable String reason,
        long nowNanos,
        long feedbackNowMillis
    ) {
        expirePendingErrorClose(nowNanos);
        AgentUiScreen screen = activeScreen;
        if (screen != null && screen.requestId().equals(requestId)) {
            activeScreen = null;
            clearPendingErrorCloseIfMatch(requestId);
            screen.receiveCloseSignal(reason);
            return;
        }
        if (screen == null) {
            PendingErrorClose pending = pendingErrorClose;
            if (pending != null && pending.requestId().equals(requestId)) {
                // 本地响应会先关屏；只允许匹配且未过期的 server close 消费一次。
                pendingErrorClose = null;
                AgentUiCloseFeedback.showForReasonAt(reason, Math.max(0L, feedbackNowMillis));
            }
        }
    }

    /** 本地响应已发出且 screen 将关闭，登记仍待 server 错误终态确认的 request。 */
    static void markAwaitingErrorClose(AgentUiScreen screen) {
        markAwaitingErrorCloseAtNanos(screen, System.nanoTime());
    }

    static void markAwaitingErrorCloseAtNanos(AgentUiScreen screen, long nowNanos) {
        if (activeScreen != screen) {
            return;
        }
        pendingErrorClose = new PendingErrorClose(screen.requestId(), nowNanos, nowNanos);
    }

    private static void expirePendingErrorClose(long nowNanos) {
        PendingErrorClose pending = pendingErrorClose;
        if (pending == null) {
            return;
        }
        long elapsedNanos = nowNanos - pending.startedAtNanos();
        if (nowNanos < pending.lastObservedNanos()
            || elapsedNanos < 0L
            || elapsedNanos >= PENDING_ERROR_CLOSE_TTL_NANOS) {
            pendingErrorClose = null;
            return;
        }
        if (nowNanos != pending.lastObservedNanos()) {
            pendingErrorClose = new PendingErrorClose(
                pending.requestId(), pending.startedAtNanos(), nowNanos);
        }
    }

    private static void clearPendingErrorCloseIfMatch(String requestId) {
        PendingErrorClose pending = pendingErrorClose;
        if (pending != null && pending.requestId().equals(requestId)) {
            pendingErrorClose = null;
        }
    }

    /** 清除存储（用于测试 / 连接断开时清理）。 */
    public static void clear() {
        activeScreen = null;
        pendingErrorClose = null;
    }

    public static void clearOnDisconnect() {
        clear();
    }

    private record PendingErrorClose(
        String requestId,
        long startedAtNanos,
        long lastObservedNanos
    ) {}
}
