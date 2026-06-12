package com.bong.client.agentui;

import org.jetbrains.annotations.Nullable;

/**
 * plan-agent-ui-data-v1 P1 — 客户端当前活跃 AgentUiScreen 单例存储。
 *
 * <p>单面板互斥：同一时刻最多一个活跃 session。
 * 新请求覆盖旧 session（server close 信号先到时由 {@link #receiveClose} 清除）。
 */
public final class AgentUiStore {
    private AgentUiStore() {}

    @Nullable
    private static volatile AgentUiScreen activeScreen = null;

    /** 设置当前活跃 screen（替换旧的，旧的由调用方负责关闭）。 */
    public static void setActive(@Nullable AgentUiScreen screen) {
        activeScreen = screen;
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
        AgentUiScreen screen = activeScreen;
        if (screen != null && screen.requestId().equals(requestId)) {
            activeScreen = null;
            screen.receiveCloseSignal();
        }
    }

    /** 清除存储（用于测试 / 连接断开时清理）。 */
    public static void clear() {
        activeScreen = null;
    }
}
