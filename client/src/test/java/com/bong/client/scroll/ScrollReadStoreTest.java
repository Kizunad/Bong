package com.bong.client.scroll;

import com.bong.client.network.ClientRequestSender;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
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
        ScrollReadStore.addListener(notified::add);
        ScrollReadStore.replace(fixture("scroll_reset"));

        ScrollReadStore.resetForTests();

        assertNull(ScrollReadStore.snapshot());
        // 监听器应被拆除：reset 后 replace 不应再通知已移除的 listener
        ScrollReadStore.replace(fixture("scroll_after_reset"));
        assertEquals(1, notified.size(), "reset 后监听器应被清空，之后的 replace 不应再通知旧 listener");
    }
}
