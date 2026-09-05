package com.bong.client.identity;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;
import java.util.logging.Level;
import java.util.logging.Logger;

/**
 * Volatile snapshot store for {@link IdentityPanelState}（plan-identity-v1 §7）。
 *
 * <p>server CustomPayload {@code bong:identity_panel_state} handler 调
 * {@link #replace(IdentityPanelState)} 推新数据；HUD / Screen 用
 * {@link #addListener(Consumer)} 订阅。
 */
public final class IdentityPanelStateStore {
    private static final Logger LOGGER =
            Logger.getLogger(IdentityPanelStateStore.class.getName());
    private static volatile IdentityPanelState snapshot = IdentityPanelState.empty();
    private static final List<Consumer<IdentityPanelState>> listeners = new CopyOnWriteArrayList<>();

    private IdentityPanelStateStore() {}

    public static IdentityPanelState snapshot() {
        return snapshot;
    }

    public static void replace(IdentityPanelState next) {
        IdentityPanelState value = next == null ? IdentityPanelState.empty() : next;
        snapshot = value;
        for (Consumer<IdentityPanelState> listener : listeners) {
            try {
                listener.accept(value);
            } catch (RuntimeException ex) {
                // listener 抛 RuntimeException 不影响其他订阅者；Error 级别（OOM /
                // StackOverflow）不接住，让它正常向上抛——掩盖致命错误反而更难排障。
                LOGGER.log(Level.WARNING, "IdentityPanelState listener threw", ex);
            }
        }
    }

    public static void addListener(Consumer<IdentityPanelState> listener) {
        if (listener != null) {
            listeners.add(listener);
        }
    }

    public static void removeListener(Consumer<IdentityPanelState> listener) {
        if (listener != null) {
            listeners.remove(listener);
        }
    }

    /**
     * plan-bughunt-client-identity-panel-stale-session-v1 — 断线时重置身份快照（生产态，
     * 供 {@code BongNetworkHandler.clearClientStateOnDisconnect()} 调用）。
     *
     * <p>{@code snapshot} 是跨 session 存活的 static；此前没有生产态清理入口，断线重连后
     * 上一 server 推送的 identity 列表 / active identity 会继续留在内存里，直到新 session
     * 首个 {@code identity_panel_state} payload 到达为止。若玩家在此期间打开
     * {@link IdentityPanelScreen}，面板会用旧快照
     * init 出按钮（回调固化旧 identityId），后续 fresh payload 只能改文字改不了按钮，造成
     * split-brain UI。复用 {@link #replace(IdentityPanelState)} 保证监听者（含面板重建）
     * 同步收到清空通知。
     */
    public static void clearOnDisconnect() {
        replace(IdentityPanelState.empty());
    }

    /** 仅供测试重置全局状态。 */
    public static void resetForTest() {
        snapshot = IdentityPanelState.empty();
        listeners.clear();
    }
}
