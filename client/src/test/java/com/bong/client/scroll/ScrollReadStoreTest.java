package com.bong.client.scroll;

import com.bong.client.network.ClientRequestSender;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * ScrollReadStore 生命周期单测（plan-scroll-reading-v1 P1）。
 *
 * <p>覆盖：open → close → 再 open 的完整生命周期、监听器通知、幂等关闭（防重复
 * ScrollReadClosed）、断线清空、resetForTests。测契约（可观察的 snapshot/网络发包），
 * 不测实现细节。
 */
class ScrollReadStoreTest {

    private record Sent(Identifier channel, String body) {}

    private final List<Sent> sent = new ArrayList<>();

    @AfterEach
    void cleanup() {
        ScrollReadStore.resetForTests();
        ClientRequestSender.resetBackendForTests();
    }

    private void installNetworkCapture() {
        ClientRequestSender.setBackendForTests(
            (channel, payload) -> sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8)))
        );
    }

    private static ScrollOpenViewModel fixture(String scrollId) {
        return new ScrollOpenViewModel(scrollId, "标题·" + scrollId, List.of("正文"));
    }

    private static ScrollReadStore.SessionToken currentToken(ScrollOpenViewModel viewModel) {
        ScrollReadStore.SessionToken token = ScrollReadStore.sessionTokenFor(viewModel);
        assertNotNull(token, "当前 viewModel 必须绑定活跃阅读会话 token");
        return token;
    }

    // ─── open→close→再open 完整生命周期 ────────────────────────────────────

    @Test
    void openCloseReopen_fullLifecycle() {
        installNetworkCapture();
        List<ScrollOpenViewModel> notified = new ArrayList<>();
        ScrollReadStore.addListener(notified::add);

        // 1. open
        ScrollOpenViewModel first = fixture("scroll_a");
        ScrollReadStore.replace(first);
        assertEquals(first, ScrollReadStore.snapshot(), "open 后 snapshot 应为第一次推入的 viewModel");
        assertEquals(1, notified.size(), "open 应通知监听器一次");
        assertEquals(first, notified.get(0));

        // 2. close
        ScrollReadStore.close();
        assertNull(ScrollReadStore.snapshot(), "close 后 snapshot 应为 null");
        assertEquals(2, notified.size(), "close 应再通知监听器一次（携带 null）");
        assertNull(notified.get(1));
        assertEquals(1, sent.size(), "close 应发出恰好一条 ScrollReadClosed");
        assertEquals(new Identifier("bong", "client_request"), sent.get(0).channel());
        assertEquals("{\"type\":\"scroll_read_closed\",\"v\":1}", sent.get(0).body());

        // 3. 再 open（不同 scrollId，验证 store 可重新填入）
        ScrollOpenViewModel second = fixture("scroll_b");
        ScrollReadStore.replace(second);
        assertEquals(second, ScrollReadStore.snapshot(), "再 open 后 snapshot 应为第二次推入的 viewModel");
        assertEquals(3, notified.size(), "再 open 应再通知监听器一次");
        assertEquals(second, notified.get(2));

        // 4. 再 close
        ScrollReadStore.close();
        assertNull(ScrollReadStore.snapshot());
        assertEquals(2, sent.size(), "第二次 close 应再发出一条 ScrollReadClosed（累计 2 条）");
    }

    // ─── close 幂等：已是空 slot 时不重复发包 ───────────────────────────────

    @Test
    void close_whenAlreadyEmpty_isNoOpAndDoesNotSendNetwork() {
        installNetworkCapture();
        assertNull(ScrollReadStore.snapshot(), "初始 snapshot 应为 null");

        ScrollReadStore.close();

        assertNull(ScrollReadStore.snapshot());
        assertTrue(sent.isEmpty(),
            "store 已空时 close() 应为 no-op，不应发出 ScrollReadClosed，实际发出=" + sent);
    }

    @Test
    void close_calledTwiceInARow_onlySendsOnce() {
        installNetworkCapture();
        ScrollReadStore.replace(fixture("scroll_dup"));

        ScrollReadStore.close();
        ScrollReadStore.close(); // 第二次 close：store 已空，应是 no-op

        assertEquals(1, sent.size(),
            "连续两次 close() 只应发出一条 ScrollReadClosed（第二次因 store 已空而 no-op），实际=" + sent);
    }

    @Test
    void closeIfCurrent_whenExpectedSessionWasReplaced_isNoOp() {
        installNetworkCapture();
        ScrollOpenViewModel first = fixture("scroll_old");
        ScrollOpenViewModel replacement = fixture("scroll_new");
        ScrollReadStore.replace(first);
        ScrollReadStore.SessionToken firstToken = currentToken(first);
        ScrollReadStore.replace(replacement);

        ScrollReadStore.closeIfCurrent(firstToken);

        assertSame(replacement, ScrollReadStore.snapshot(),
            "旧 screen 的取消回调不得清掉后来替换的新阅读会话");
        assertTrue(sent.isEmpty(), "旧 screen 的取消回调不得替新会话发送终态，实际=" + sent);
    }

    @Test
    void closeIfCurrent_whenTransportReplacesSession_preservesReplacement() {
        ScrollOpenViewModel current = fixture("scroll_current");
        ScrollOpenViewModel replacement = fixture("scroll_reentrant");
        ScrollReadStore.replace(current);
        ScrollReadStore.SessionToken currentToken = currentToken(current);
        ClientRequestSender.setBackendForTests((channel, payload) -> {
            sent.add(new Sent(channel, new String(payload, StandardCharsets.UTF_8)));
            ScrollReadStore.replace(replacement);
        });

        ScrollReadStore.closeIfCurrent(currentToken);

        assertSame(replacement, ScrollReadStore.snapshot(),
            "终态发送期间重入的新会话不得被旧关闭流程清空");
        assertEquals(1, sent.size(), "当前会话仍应恰好发送一条终态请求");
    }

    @Test
    void replace_sameScrollDuringActiveSessionRefreshesSnapshotAndPreservesToken() {
        List<ScrollOpenViewModel> notified = new ArrayList<>();
        ScrollReadStore.addListener(notified::add);
        ScrollOpenViewModel current = fixture("scroll_same");
        ScrollOpenViewModel refresh = new ScrollOpenViewModel(
            "scroll_same", "刷新标题", List.of("刷新正文")
        );
        ScrollReadStore.replace(current);
        ScrollReadStore.SessionToken tokenBeforeRefresh = currentToken(current);

        ScrollReadStore.replace(refresh);

        assertSame(refresh, ScrollReadStore.snapshot(),
            "同一卷连续 scroll_open 应更新渲染快照，不能丢弃服务端的新正文");
        assertSame(refresh, notified.get(1), "同卷刷新通知必须携带最新渲染快照");
        assertSame(tokenBeforeRefresh, currentToken(refresh),
            "同一活跃会话内的同卷刷新必须保留 token，使现有 screen 仍拥有关闭权");
        assertNull(ScrollReadStore.sessionTokenFor(current),
            "刷新后的旧 viewModel 不应再被当作当前渲染快照");
    }

    @Test
    void replace_sameScrollAfterEmptyStateCreatesNewSessionIdentity() {
        ScrollOpenViewModel oldSession = fixture("scroll_same");
        ScrollOpenViewModel newSession = fixture("scroll_same");
        ScrollReadStore.replace(oldSession);
        ScrollReadStore.SessionToken oldToken = currentToken(oldSession);
        ScrollReadStore.clearOnDisconnect();

        ScrollReadStore.replace(newSession);

        assertSame(newSession, ScrollReadStore.snapshot(),
            "经过空态后重开的同一卷必须创建新会话，旧 screen 不能获得其关闭权");
        assertNotSame(oldToken, currentToken(newSession),
            "经过空态后重开必须轮换 token，不能只靠 scrollId 或 viewModel 身份区分会话");
    }

    @Test
    void closeIfCurrent_reusedViewModelAfterEmptyStateRejectsOldToken() {
        installNetworkCapture();
        ScrollOpenViewModel reused = fixture("scroll_reused");
        ScrollReadStore.replace(reused);
        ScrollReadStore.SessionToken oldToken = currentToken(reused);
        ScrollReadStore.clearOnDisconnect();
        ScrollReadStore.replace(reused);

        ScrollReadStore.closeIfCurrent(oldToken);

        assertSame(reused, ScrollReadStore.snapshot(),
            "同一 viewModel 实例经历空态后属于新会话，旧 token 不得通过 ABA 误结算");
        assertTrue(sent.isEmpty(), "旧 token 不得替重开会话发送 scroll_read_closed，实际=" + sent);
    }

    @Test
    void closeIfCurrent_concurrentClosersSendExactlyOneTerminalRequest() throws Exception {
        ScrollOpenViewModel current = fixture("scroll_concurrent");
        ScrollReadStore.replace(current);
        ScrollReadStore.SessionToken currentToken = currentToken(current);
        AtomicInteger sends = new AtomicInteger();
        CountDownLatch firstSendEntered = new CountDownLatch(1);
        CountDownLatch releaseSend = new CountDownLatch(1);
        ClientRequestSender.setBackendForTests((channel, payload) -> {
            sends.incrementAndGet();
            firstSendEntered.countDown();
            try {
                if (!releaseSend.await(5, TimeUnit.SECONDS)) {
                    throw new AssertionError("等待并发关闭验证超时");
                }
            } catch (InterruptedException exception) {
                Thread.currentThread().interrupt();
                throw new AssertionError("并发关闭验证被中断", exception);
            }
        });
        ExecutorService executor = Executors.newFixedThreadPool(2);
        try {
            Future<?> first = executor.submit(() -> ScrollReadStore.closeIfCurrent(currentToken));
            assertTrue(firstSendEntered.await(5, TimeUnit.SECONDS), "首个关闭应进入终态发送");
            Future<?> second = executor.submit(() -> ScrollReadStore.closeIfCurrent(currentToken));

            second.get(5, TimeUnit.SECONDS);
            releaseSend.countDown();
            first.get(5, TimeUnit.SECONDS);
        } finally {
            releaseSend.countDown();
            executor.shutdownNow();
        }

        assertNull(ScrollReadStore.snapshot(), "并发关闭后会话必须处于空态");
        assertEquals(1, sends.get(), "并发关闭同一会话也只能发送一条 scroll_read_closed");
    }

    @Test
    void sessionSnapshotsInvalidateAcrossRefreshClearAndReopen() {
        List<ScrollReadStore.ActiveSession> notified = new ArrayList<>();
        ScrollReadStore.addSessionListener(notified::add);
        ScrollOpenViewModel reused = fixture("scroll_session");
        ScrollReadStore.replace(reused);
        ScrollReadStore.ActiveSession opened = notified.get(0);
        ScrollOpenViewModel refresh = new ScrollOpenViewModel(
            "scroll_session", "刷新标题", List.of("刷新正文")
        );

        ScrollReadStore.replace(refresh);
        ScrollReadStore.ActiveSession refreshed = notified.get(1);
        ScrollReadStore.clearOnDisconnect();
        assertTrue(ScrollReadStore.isCurrent(null), "清空后空态任务必须是当前状态");
        ScrollReadStore.replace(reused);
        ScrollReadStore.ActiveSession reopened = notified.get(3);

        assertFalse(ScrollReadStore.isCurrent(opened), "同卷刷新后旧异步 open 任务必须失效");
        assertFalse(ScrollReadStore.isCurrent(refreshed), "清空后旧异步 refresh 任务必须失效");
        assertFalse(ScrollReadStore.isCurrent(null), "重开后迟到的异步 close 任务必须失效");
        assertTrue(ScrollReadStore.isCurrent(reopened), "只有最新重开任务可以驱动 screen");
        assertSame(opened.token(), refreshed.token(), "同卷刷新必须保留会话 token");
        assertNotSame(refreshed.token(), reopened.token(), "经过空态重开必须轮换会话 token");
        assertNull(notified.get(2), "clearOnDisconnect 必须发布明确空态");
    }

    // ─── 监听器通知 ─────────────────────────────────────────────────────

    @Test
    void replaceNotifiesListeners() {
        List<ScrollOpenViewModel> notified = new ArrayList<>();
        ScrollReadStore.addListener(notified::add);

        ScrollOpenViewModel offer = fixture("scroll_notify");
        ScrollReadStore.replace(offer);

        assertEquals(1, notified.size());
        assertEquals(offer, notified.get(0));
        assertEquals(offer, ScrollReadStore.snapshot());
    }

    @Test
    void removeListener_stopsFurtherNotifications() {
        List<ScrollOpenViewModel> notified = new ArrayList<>();
        java.util.function.Consumer<ScrollOpenViewModel> listener = notified::add;
        ScrollReadStore.addListener(listener);
        ScrollReadStore.replace(fixture("scroll_1"));
        assertEquals(1, notified.size());

        ScrollReadStore.removeListener(listener);
        ScrollReadStore.replace(fixture("scroll_2"));

        assertEquals(1, notified.size(),
            "移除监听器后不应再收到通知，实际大小=" + notified.size());
    }

    // ─── 断线兜底 ──────────────────────────────────────────────────────

    @Test
    void clearOnDisconnect_clearsSnapshotButKeepsListeners() {
        List<ScrollOpenViewModel> notified = new ArrayList<>();
        ScrollReadStore.addListener(notified::add);
        ScrollReadStore.replace(fixture("scroll_disc"));

        ScrollReadStore.clearOnDisconnect();

        assertNull(ScrollReadStore.snapshot(), "断线后 snapshot 应清空");
        // 监听器应保留：再次 replace 仍应通知（2 次：open + disconnect-clear，第 3 次是新 open）
        ScrollReadStore.replace(fixture("scroll_after_disc"));
        assertEquals(3, notified.size(),
            "断线兜底不应拆掉监听器——之后的 replace 仍应正常通知，实际通知次数=" + notified.size());
    }

    @Test
    void clearOnDisconnect_doesNotSendNetworkRequest() {
        // 区别于 close()：断线时不应尝试回传 C2S（连接已经没了）。
        installNetworkCapture();
        ScrollReadStore.replace(fixture("scroll_disc2"));

        ScrollReadStore.clearOnDisconnect();

        assertTrue(sent.isEmpty(),
            "clearOnDisconnect 不应发出任何 C2S 请求，实际发出=" + sent);
    }

    // ─── resetForTests ───────────────────────────────────────────────────

    @Test
    void resetForTests_clearsSnapshotAndListeners() {
        List<ScrollOpenViewModel> notified = new ArrayList<>();
        List<ScrollReadStore.ActiveSession> sessionNotified = new ArrayList<>();
        ScrollReadStore.addListener(notified::add);
        ScrollReadStore.addSessionListener(sessionNotified::add);
        ScrollReadStore.replace(fixture("scroll_reset"));

        ScrollReadStore.resetForTests();

        assertNull(ScrollReadStore.snapshot());
        // 监听器应被拆除：reset 后 replace 不应再通知已移除的 listener
        ScrollReadStore.replace(fixture("scroll_after_reset"));
        assertEquals(1, notified.size(), "reset 后监听器应被清空，之后的 replace 不应再通知旧 listener");
        assertEquals(1, sessionNotified.size(), "reset 后 session listener 也必须清空，避免测试/重连串会话");
    }
}
