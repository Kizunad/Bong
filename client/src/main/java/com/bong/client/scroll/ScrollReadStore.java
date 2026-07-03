package com.bong.client.scroll;

import com.bong.client.network.ClientRequestSender;

import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

/**
 * 当前打开的阅读屏快照（plan-scroll-reading-v1 P1）——单 slot，同一时刻最多一卷在读。
 *
 * <p>跟 {@code InsightOfferStore} 一样：{@code volatile} 快照 + 监听器列表，供
 * {@link ScrollReadScreenBootstrap} 订阅以驱动开屏/关屏。
 */
public final class ScrollReadStore {
    private static volatile ScrollOpenViewModel snapshot = null;
    private static final CopyOnWriteArrayList<Consumer<ScrollOpenViewModel>> listeners =
        new CopyOnWriteArrayList<>();

    private ScrollReadStore() {
    }

    public static ScrollOpenViewModel snapshot() {
        return snapshot;
    }

    /** 推入新的 ScrollOpen（null = 当前阅读屏已结算/取消）。 */
    public static void replace(ScrollOpenViewModel next) {
        snapshot = next;
        for (Consumer<ScrollOpenViewModel> listener : listeners) {
            listener.accept(next);
        }
    }

    public static void addListener(Consumer<ScrollOpenViewModel> listener) {
        listeners.add(listener);
    }

    public static void removeListener(Consumer<ScrollOpenViewModel> listener) {
        listeners.remove(listener);
    }

    /**
     * 玩家关闭阅读屏（ESC / 关闭按钮）：回传 server {@code ScrollReadClosed} 并清空当前 slot。
     *
     * <p>已是空 slot 时视为幂等 no-op——防止重复 ESC / 关闭事件对同一次阅读会话发出多条
     * {@code ScrollReadClosed}（例如 Screen.close() 与 store 监听器都可能触发关闭路径）。
     */
    public static void close() {
        if (snapshot == null) {
            return;
        }
        ClientRequestSender.sendScrollReadClosed();
        replace(null);
    }

    /** 断线时调用：仅清当前快照，保留监听器（对齐 InsightOfferStore.clearOnDisconnect）。 */
    public static void clearOnDisconnect() {
        replace(null);
    }

    public static void resetForTests() {
        snapshot = null;
        listeners.clear();
    }
}
