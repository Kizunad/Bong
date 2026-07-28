package com.bong.client.scroll;

import com.bong.client.network.ClientRequestSender;

import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.Consumer;

/**
 * 当前打开的阅读屏快照（plan-scroll-reading-v1 P1）——单 slot，同一时刻最多一卷在读。
 *
 * <p>view model 只承载渲染数据；每次从空态进入阅读态都会创建不可复用的会话 token。
 * 同一阅读会话内的同卷刷新保留 token，供 screen 精确结算自己所属的会话。
 */
public final class ScrollReadStore {
    private static final AtomicReference<ActiveSession> activeSession = new AtomicReference<>();
    private static final CopyOnWriteArrayList<Consumer<ScrollOpenViewModel>> listeners =
        new CopyOnWriteArrayList<>();
    private static final CopyOnWriteArrayList<Consumer<ActiveSession>> sessionListeners =
        new CopyOnWriteArrayList<>();

    private ScrollReadStore() {
    }

    public static ScrollOpenViewModel snapshot() {
        ActiveSession current = activeSession.get();
        return current == null ? null : current.viewModel();
    }

    /** 推入新的 ScrollOpen（null = 当前阅读屏已结算/取消）。 */
    public static void replace(ScrollOpenViewModel next) {
        ActiveSession published;
        if (next == null) {
            activeSession.set(null);
            published = null;
        } else {
            published = activeSession.updateAndGet(current -> sameActiveScroll(current, next)
                ? new ActiveSession(current.token(), next)
                : new ActiveSession(new SessionToken(), next));
        }
        notifyListeners(published);
    }

    private static boolean sameActiveScroll(ActiveSession current, ScrollOpenViewModel next) {
        return current != null && current.viewModel().scrollId().equals(next.scrollId());
    }

    private static void notifyListeners(ActiveSession next) {
        for (Consumer<ActiveSession> listener : sessionListeners) {
            listener.accept(next);
        }
        ScrollOpenViewModel viewModel = next == null ? null : next.viewModel();
        for (Consumer<ScrollOpenViewModel> listener : listeners) {
            listener.accept(viewModel);
        }
    }

    public static void addListener(Consumer<ScrollOpenViewModel> listener) {
        listeners.add(listener);
    }

    public static void removeListener(Consumer<ScrollOpenViewModel> listener) {
        listeners.remove(listener);
    }

    static void addSessionListener(Consumer<ActiveSession> listener) {
        sessionListeners.add(listener);
    }

    static boolean isCurrent(ActiveSession expected) {
        return activeSession.get() == expected;
    }

    static SessionToken sessionTokenFor(ScrollOpenViewModel expected) {
        ActiveSession current = activeSession.get();
        return current != null && current.viewModel() == expected ? current.token() : null;
    }

    /**
     * 玩家关闭阅读屏（ESC / 关闭按钮）：回传 server {@code ScrollReadClosed} 并清空当前 slot。
     *
     * <p>已是空 slot 时视为幂等 no-op——防止重复 ESC / 关闭事件对同一次阅读会话发出多条
     * {@code ScrollReadClosed}（例如 Screen.close() 与 store 监听器都可能触发关闭路径）。
     */
    public static void close() {
        ActiveSession current = activeSession.get();
        closeIfCurrent(current == null ? null : current.token());
    }

    static void closeIfCurrent(SessionToken expected) {
        if (expected == null) {
            return;
        }
        while (true) {
            ActiveSession current = activeSession.get();
            if (current == null || current.token() != expected) {
                return;
            }
            if (activeSession.compareAndSet(current, null)) {
                break;
            }
        }
        // 关闭是本地不可逆终态：即使当前 play transport 已断开或拒绝 payload，也不能让
        // 已经视觉关闭的阅读会话继续残留在 store。server 侧断线清理负责兜底 marker。
        notifyListeners(null);
        ClientRequestSender.sendScrollReadClosed();
    }

    /**
     * DISCONNECT 的同步 ABA 栅栏：仅轮换当前阅读会话身份，不清 payload、不开关屏也不通知监听器。
     *
     * <p>已经排队的开屏任务持有旧 {@link ActiveSession}，旧 screen 持有旧
     * {@link SessionToken}；轮换后两者都会 fail closed。真正的数据清理由集中 lifecycle
     * registry 随后调用 {@link #clearOnDisconnect()}，不能由这个同步屏障重复执行。
     */
    static void invalidateSessionIdentityOnDisconnect() {
        activeSession.updateAndGet(current -> current == null
            ? null
            : new ActiveSession(new SessionToken(), current.viewModel()));
    }

    /** 由集中 lifecycle registry 调用：仅清当前快照，保留监听器。 */
    public static void clearOnDisconnect() {
        replace(null);
    }

    public static void resetForTests() {
        activeSession.set(null);
        listeners.clear();
        sessionListeners.clear();
    }

    static final class SessionToken {
        private SessionToken() {
        }
    }

    static record ActiveSession(SessionToken token, ScrollOpenViewModel viewModel) {
    }
}
