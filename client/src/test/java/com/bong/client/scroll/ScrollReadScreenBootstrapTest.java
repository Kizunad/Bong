package com.bong.client.scroll;

import net.minecraft.client.gui.screen.Screen;
import net.minecraft.text.Text;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ScrollReadScreenBootstrapTest {
    @AfterEach
    void cleanup() {
        ScrollReadStore.resetForTests();
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

    private static ScrollOpenViewModel fixture(String body) {
        return new ScrollOpenViewModel("scroll_same", "同卷", List.of(body));
    }

    private static final class DummyScreen extends Screen {
        private DummyScreen() {
            super(Text.literal("dummy"));
        }
    }
}
