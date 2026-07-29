package com.bong.client.scroll;

import com.bong.client.BongNetworkHandler;
import com.bong.client.ui.ClientConnectionStatusStore;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.text.Text;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Consumer;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ScrollReadScreenBootstrapTest {
    @AfterEach
    void cleanup() {
        ScrollReadStore.resetForTests();
        ClientConnectionStatusStore.resetForTests();
    }

    @Test
    void storeCallbackWithoutLiveClientIsNoOpForOpenAndClear() {
        List<ScrollReadStore.ActiveSession> sessions = new ArrayList<>();
        ScrollReadStore.addSessionListener(sessions::add);
        ScrollReadStore.replace(fixture("正文"));

        assertDoesNotThrow(() -> ScrollReadScreenBootstrap.onStoreChanged(sessions.get(0)),
            "无 live MinecraftClient 时 open 回调必须安全 short-circuit");
        assertDoesNotThrow(() -> ScrollReadScreenBootstrap.onStoreChanged(null),
            "无 live MinecraftClient 时 clear 回调必须安全 short-circuit");
    }

    @Test
    void sameSessionRefreshKeepsCurrentAndPendingScreenIdentity() {
        List<ScrollReadStore.ActiveSession> sessions = new ArrayList<>();
        ScrollReadStore.addSessionListener(sessions::add);
        ScrollOpenViewModel original = fixture("原正文");
        ScrollReadStore.replace(original);
        ScrollReadScreen screen = new ScrollReadScreen(original);
        ScrollReadStore.replace(fixture("刷新正文"));

        ScrollReadStore.ActiveSession refreshed = sessions.get(1);

        assertTrue(ScrollReadScreenBootstrap.belongsToSession(screen, refreshed),
            "同一会话内的同卷刷新必须继续认领当前或 pending screen");
    }

    @Test
    void emptyStateThenSameScrollReopenRejectsOldScreenIdentity() {
        List<ScrollReadStore.ActiveSession> sessions = new ArrayList<>();
        ScrollReadStore.addSessionListener(sessions::add);
        ScrollOpenViewModel reused = fixture("正文");
        ScrollReadStore.replace(reused);
        ScrollReadScreen oldScreen = new ScrollReadScreen(reused);
        ScrollReadStore.clearOnDisconnect();
        ScrollReadStore.replace(reused);

        ScrollReadStore.ActiveSession reopened = sessions.get(2);

        assertFalse(ScrollReadScreenBootstrap.belongsToSession(oldScreen, reopened),
            "经历空态后同 scrollId 也属于新会话，bootstrap 必须换屏而不是复用旧 token");
    }

    @Test
    void unrelatedScreenNeverClaimsScrollSession() {
        List<ScrollReadStore.ActiveSession> sessions = new ArrayList<>();
        ScrollReadStore.addSessionListener(sessions::add);
        ScrollReadStore.replace(fixture("正文"));

        assertFalse(ScrollReadScreenBootstrap.belongsToSession(
            new DummyScreen(), sessions.get(0)), "无关 screen 不得阻止当前阅读会话开屏");
        assertFalse(ScrollReadScreenBootstrap.belongsToSession(null, sessions.get(0)),
            "空 screen 不得被误判为当前阅读会话");
    }

    @Test
    void centralRegistryClearInvalidatesQueuedOpenBeforeItCanSetScreen() {
        List<ScrollReadStore.ActiveSession> sessions = new ArrayList<>();
        List<Screen> setScreens = new ArrayList<>();
        ScrollReadStore.addSessionListener(sessions::add);
        ScrollReadStore.replace(fixture("正文"));
        ScrollReadStore.ActiveSession queuedOpen = sessions.get(0);

        ScrollReadStore.clearOnDisconnect();
        ScrollReadScreenBootstrap.applyStoreChange(null, null, setScreens::add, queuedOpen);

        assertFalse(ScrollReadStore.isCurrent(queuedOpen),
            "中央 registry 清理必须使断线前排队的阅读 session 失效");
        assertTrue(setScreens.isEmpty(),
            "断线前排队的 open 任务不得在中央清理后开屏，实际 setScreen=" + setScreens);
    }

    @Test
    void lateOldHandlerDisconnectCannotRevokeNewScrollSessionOwnership() {
        Object oldHandler = new Object();
        ClientConnectionStatusStore.initializeSession(oldHandler);
        assertTrue(ClientConnectionStatusStore.activateSession(oldHandler, 1_000L),
            "测试前置：handler A 必须成为 active session");

        List<ScrollReadStore.ActiveSession> sessions = new ArrayList<>();
        List<Runnable> disconnectTasks = new ArrayList<>();
        ScrollReadStore.addSessionListener(sessions::add);
        invokeDisconnectSession(
            oldHandler,
            1_100L,
            ScrollReadStore::clearOnDisconnect,
            disconnectTasks::add
        );

        Object newHandler = new Object();
        ClientConnectionStatusStore.SessionToken newConnectionToken =
            ClientConnectionStatusStore.initializeSession(newHandler);
        assertTrue(ClientConnectionStatusStore.activateSession(newHandler, 2_000L),
            "测试前置：handler B JOIN 必须先成为 active session");
        ScrollOpenViewModel freshModel = fixture("新正文");
        ScrollReadStore.replace(freshModel);
        ScrollReadStore.ActiveSession freshSession = sessions.get(0);
        ScrollReadScreen freshScreen = new ScrollReadScreen(freshModel, freshSession.token());

        disconnectTasks.remove(0).run();

        assertTrue(ClientConnectionStatusStore.isActiveSession(newConnectionToken),
            "handler A 的迟到 DISCONNECT 不得使 handler B token 失活");
        assertTrue(ScrollReadStore.isCurrent(freshSession),
            "handler A 的迟到 DISCONNECT 不得轮换或清除 handler B 的阅读身份");
        assertTrue(ScrollReadScreenBootstrap.belongsToSession(freshScreen, freshSession),
            "handler B 已打开的阅读屏必须继续拥有当前会话，不能被旧 A 撤销 token");
        assertSame(freshModel, ScrollReadStore.snapshot(),
            "handler A 的迟到 DISCONNECT 不得清除 handler B 的阅读数据");
    }

    @Test
    void registryClearAllowsFreshSessionToOpenAndRejectsOldScreenClose() {
        List<ScrollReadStore.ActiveSession> sessions = new ArrayList<>();
        List<Screen> setScreens = new ArrayList<>();
        ScrollReadStore.addSessionListener(sessions::add);
        ScrollOpenViewModel oldModel = fixture("旧正文");
        ScrollReadStore.replace(oldModel);
        ScrollReadStore.ActiveSession oldSession = sessions.get(0);
        ScrollReadScreen oldScreen = new ScrollReadScreen(oldModel, oldSession.token());

        ScrollReadStore.clearOnDisconnect();
        ScrollOpenViewModel freshModel = fixture("新正文");
        ScrollReadStore.replace(freshModel);
        ScrollReadStore.ActiveSession freshSession = sessions.get(2);

        ScrollReadScreenBootstrap.applyStoreChange(null, null, setScreens::add, freshSession);
        oldScreen.close();

        assertEquals(1, setScreens.size(),
            "集中 clear 后新 session 的当前 open 必须恰好开一次屏，实际=" + setScreens);
        assertTrue(ScrollReadScreenBootstrap.belongsToSession(setScreens.get(0), freshSession),
            "新开的 screen 必须绑定 fresh token，不能复用断线前身份");
        assertSame(freshModel, ScrollReadStore.snapshot(),
            "旧 screen 关闭不得结算集中 clear 后重开的 fresh session");
    }

    @Test
    void staleClearNotificationAfterReopenDoesNotCloseNewSession() {
        List<ScrollReadStore.ActiveSession> sessions = new ArrayList<>();
        List<Screen> setScreens = new ArrayList<>();
        ScrollReadStore.addSessionListener(sessions::add);
        ScrollReadStore.replace(fixture("旧会话"));
        ScrollReadStore.clearOnDisconnect();
        ScrollOpenViewModel reopenedModel = fixture("重开会话");
        ScrollReadStore.replace(reopenedModel);
        ScrollReadStore.ActiveSession reopened = sessions.get(2);
        ScrollReadScreen reopenedScreen = new ScrollReadScreen(reopenedModel, reopened.token());

        ScrollReadScreenBootstrap.applyStoreChange(
            reopenedScreen,
            null,
            setScreens::add,
            null
        );

        assertSame(reopenedModel, ScrollReadStore.snapshot(),
            "迟到的 registry clear 通知不得清理后来重开的 session");
        assertTrue(setScreens.isEmpty(),
            "迟到的 registry clear 通知不得关闭新 screen，实际 setScreen=" + setScreens);
    }

    @Test
    void currentOpenTaskCreatesExactlyOneSessionBoundScreen() {
        List<ScrollReadStore.ActiveSession> sessions = new ArrayList<>();
        List<Screen> setScreens = new ArrayList<>();
        ScrollReadStore.addSessionListener(sessions::add);
        ScrollOpenViewModel model = fixture("正文");
        ScrollReadStore.replace(model);
        ScrollReadStore.ActiveSession current = sessions.get(0);

        ScrollReadScreenBootstrap.applyStoreChange(null, null, setScreens::add, current);

        assertEquals(1, setScreens.size(),
            "当前 open 任务必须恰好请求一次 setScreen，实际=" + setScreens);
        assertTrue(setScreens.get(0) instanceof ScrollReadScreen,
            "当前 open 任务必须创建 ScrollReadScreen，实际=" + setScreens.get(0));
        assertTrue(ScrollReadScreenBootstrap.belongsToSession(setScreens.get(0), current),
            "新 screen 必须绑定当前不可复用 token，实际 screen=" + setScreens.get(0));
    }

    private static void invokeDisconnectSession(
        Object handler,
        long disconnectedAtMs,
        Runnable cleanupTask,
        Consumer<Runnable> clientExecutor
    ) {
        try {
            Method method = BongNetworkHandler.class.getDeclaredMethod(
                "disconnectSession", Object.class, long.class, Runnable.class, Consumer.class);
            method.setAccessible(true);
            method.invoke(null, handler, disconnectedAtMs, cleanupTask, clientExecutor);
        } catch (ReflectiveOperationException exception) {
            throw new AssertionError("无法调用 production disconnect token gate", exception);
        }
    }

    private static ScrollOpenViewModel fixture(String body) {
        return new ScrollOpenViewModel("scroll_same", "同卷", List.of(body));
    }

    private static final class DummyScreen extends Screen {
        private DummyScreen() {
            super(Text.literal("dummy"));
        }
    }
}
