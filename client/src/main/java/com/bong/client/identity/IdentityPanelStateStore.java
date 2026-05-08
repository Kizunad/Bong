package com.bong.client.identity;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

/**
 * Volatile snapshot store for {@link IdentityPanelState}（plan-identity-v1 §7）。
 *
 * <p>server CustomPayload {@code bong:identity_panel_state} handler 调
 * {@link #replace(IdentityPanelState)} 推新数据；HUD / Screen 用
 * {@link #addListener(Consumer)} 订阅。
 */
public final class IdentityPanelStateStore {
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
            } catch (Throwable ignore) {
                // listener 抛异常不影响其他订阅者
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

    /** 仅供测试重置全局状态。 */
    public static void resetForTest() {
        snapshot = IdentityPanelState.empty();
        listeners.clear();
    }
}
