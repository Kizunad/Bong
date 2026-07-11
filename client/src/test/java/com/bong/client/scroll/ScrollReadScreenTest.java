package com.bong.client.scroll;

import com.bong.client.network.ClientRequestSender;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * ScrollReadScreen 纯函数式渲染描述单测（plan-scroll-reading-v1 P1）。
 *
 * <p>{@link ScrollReadScreen#describe(ScrollOpenViewModel, int)} 不依赖 owo/MC 运行时，
 * 用于在无头环境验证分页边界渲染内容（1 页 / 首页 / 末页 / 越界钳位）而无需完整 UI 启动。
 *
 * <p>可复用性验收：describe() 的输出只取决于 viewModel 的 title/bodyPages 字段，
 * 不 hardcode 任何具体残卷内容。
 */
class ScrollReadScreenTest {
    private final List<String> sentPayloads = new ArrayList<>();

    @AfterEach
    void cleanup() {
        ScrollReadStore.resetForTests();
        ClientRequestSender.resetBackendForTests();
    }

    private static ScrollOpenViewModel multiPage() {
        return new ScrollOpenViewModel(
            "scroll_x", "《测试残卷》",
            List.of("第一页正文", "第二页正文", "第三页正文")
        );
    }

    // ─── 1 页边界：不显示页码指示器 ────────────────────────────────────

    @Test
    void singlePage_doesNotShowPageIndicator() {
        ScrollOpenViewModel vm = new ScrollOpenViewModel("scroll_single", "单页残卷", List.of("唯一正文"));
        ScrollReadScreen.RenderContent content = ScrollReadScreen.describe(vm, 0);

        assertFalse(content.lines().stream().anyMatch(line -> line.contains("/")),
            "单页时不应出现「x / y」页码指示器，实际=" + content.lines());
        assertTrue(content.lines().contains("唯一正文"));
    }

    @Test
    void singlePage_outOfRangeRequestClampsToOnlyPage() {
        ScrollOpenViewModel vm = new ScrollOpenViewModel("scroll_single", "单页残卷", List.of("唯一正文"));
        ScrollReadScreen.RenderContent content = ScrollReadScreen.describe(vm, 99);

        assertTrue(content.lines().contains("唯一正文"),
            "越界页请求应钳到唯一一页，实际=" + content.lines());
    }

    // ─── 多页边界：首页 / 末页 / 中间页 ─────────────────────────────────

    @Test
    void multiPage_firstPage_showsFirstPageContentAndIndicator() {
        ScrollReadScreen.RenderContent content = ScrollReadScreen.describe(multiPage(), 0);

        assertTrue(content.lines().contains("第一页正文"));
        assertTrue(content.lines().contains("1 / 3"),
            "首页页码指示器应为 '1 / 3'，实际=" + content.lines());
    }

    @Test
    void multiPage_lastPage_showsLastPageContentAndIndicator() {
        ScrollReadScreen.RenderContent content = ScrollReadScreen.describe(multiPage(), 2);

        assertTrue(content.lines().contains("第三页正文"));
        assertTrue(content.lines().contains("3 / 3"),
            "末页页码指示器应为 '3 / 3'，实际=" + content.lines());
    }

    @Test
    void multiPage_negativeIndexClampsToFirstPage() {
        ScrollReadScreen.RenderContent content = ScrollReadScreen.describe(multiPage(), -1);

        assertTrue(content.lines().contains("第一页正文"),
            "负数页请求应钳到首页，实际=" + content.lines());
        assertTrue(content.lines().contains("1 / 3"));
    }

    @Test
    void multiPage_overflowIndexClampsToLastPage() {
        ScrollReadScreen.RenderContent content = ScrollReadScreen.describe(multiPage(), 999);

        assertTrue(content.lines().contains("第三页正文"),
            "越界页请求应钳到末页，实际=" + content.lines());
        assertTrue(content.lines().contains("3 / 3"));
    }

    @Test
    void describe_includesTitleAndCloseAffordance() {
        ScrollReadScreen.RenderContent content = ScrollReadScreen.describe(multiPage(), 0);

        assertEquals("《测试残卷》", content.lines().get(0),
            "首行应为卷名标题，实际=" + content.lines());
        assertTrue(content.lines().contains("[ 合上卷轴 ]"),
            "应包含关闭提示行，实际=" + content.lines());
    }

    @Test
    void close_staleScreenDoesNotSettleReplacementSession() {
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sentPayloads.add(new String(payload, java.nio.charset.StandardCharsets.UTF_8)));
        ScrollOpenViewModel oldOffer = new ScrollOpenViewModel("scroll_old", "旧卷", List.of("旧正文"));
        ScrollOpenViewModel replacement = new ScrollOpenViewModel("scroll_new", "新卷", List.of("新正文"));
        ScrollReadStore.replace(oldOffer);
        ScrollReadScreen oldScreen = new ScrollReadScreen(oldOffer);
        ScrollReadStore.replace(replacement);

        oldScreen.close();

        assertSame(replacement, ScrollReadStore.snapshot(),
            "旧 screen 的普通 close 不得结算后来替换的新阅读会话");
        assertTrue(sentPayloads.isEmpty(),
            "旧 screen 的普通 close 不得替新会话发送 scroll_read_closed，实际=" + sentPayloads);
    }

    @Test
    void close_currentScreenSettlesItsSessionExactlyOnce() {
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sentPayloads.add(new String(payload, java.nio.charset.StandardCharsets.UTF_8)));
        ScrollOpenViewModel offer = new ScrollOpenViewModel("scroll_current", "当前卷", List.of("正文"));
        ScrollReadStore.replace(offer);
        ScrollReadScreen screen = new ScrollReadScreen(offer);

        screen.close();
        screen.close();

        assertNull(ScrollReadStore.snapshot(), "当前 screen 关闭后必须清空对应阅读会话");
        assertEquals(List.of("{\"type\":\"scroll_read_closed\",\"v\":1}"), sentPayloads,
            "当前 screen 重复关闭必须恰好发送一条 scroll_read_closed");
    }

    @Test
    void close_sameScrollRefreshSettlesCanonicalActiveSession() {
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sentPayloads.add(new String(payload, java.nio.charset.StandardCharsets.UTF_8)));
        ScrollOpenViewModel original = new ScrollOpenViewModel("scroll_same", "同卷", List.of("原正文"));
        ScrollReadStore.replace(original);
        ScrollReadScreen retainedScreen = new ScrollReadScreen(original);
        ScrollReadStore.replace(new ScrollOpenViewModel("scroll_same", "同卷", List.of("刷新正文")));

        retainedScreen.close();

        assertNull(ScrollReadStore.snapshot(), "同卷刷新保留的 screen 仍必须能结算 canonical 会话");
        assertEquals(List.of("{\"type\":\"scroll_read_closed\",\"v\":1}"), sentPayloads,
            "同卷刷新后关闭必须恰好发送一条 scroll_read_closed");
    }

    @Test
    void close_oldScreenDoesNotSettleReopenedSameScrollSession() {
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sentPayloads.add(new String(payload, java.nio.charset.StandardCharsets.UTF_8)));
        ScrollOpenViewModel oldSession = new ScrollOpenViewModel("scroll_same", "同卷", List.of("旧会话"));
        ScrollReadStore.replace(oldSession);
        ScrollReadScreen oldScreen = new ScrollReadScreen(oldSession);
        ScrollReadStore.clearOnDisconnect();
        ScrollOpenViewModel reopenedSession = new ScrollOpenViewModel("scroll_same", "同卷", List.of("新会话"));
        ScrollReadStore.replace(reopenedSession);

        oldScreen.close();

        assertSame(reopenedSession, ScrollReadStore.snapshot(),
            "经历空态后重开的同卷是新会话，旧 screen 不得结算");
        assertTrue(sentPayloads.isEmpty(), "旧 screen 不得替重开的同卷会话发送终态");
    }

    @Test
    void close_oldScreenDoesNotSettleReopenedSessionWhenViewModelInstanceIsReused() {
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sentPayloads.add(new String(payload, java.nio.charset.StandardCharsets.UTF_8)));
        ScrollOpenViewModel reusedViewModel =
            new ScrollOpenViewModel("scroll_reused", "同卷", List.of("正文"));
        ScrollReadStore.replace(reusedViewModel);
        ScrollReadScreen oldScreen = new ScrollReadScreen(reusedViewModel);
        ScrollReadStore.clearOnDisconnect();
        ScrollReadStore.replace(reusedViewModel);

        oldScreen.close();

        assertSame(reusedViewModel, ScrollReadStore.snapshot(),
            "经历空态后即使复用同一 viewModel 实例也必须创建新会话，旧 screen 不得结算");
        assertTrue(sentPayloads.isEmpty(), "旧 screen 不得依靠 ABA 对象身份误发新会话终态");
    }

    @Test
    void removed_currentScreenSettlesItsSessionExactlyOnce() {
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sentPayloads.add(new String(payload, java.nio.charset.StandardCharsets.UTF_8)));
        ScrollOpenViewModel offer = new ScrollOpenViewModel("scroll_removed", "当前卷", List.of("正文"));
        ScrollReadStore.replace(offer);
        ScrollReadScreen screen = new ScrollReadScreen(offer);

        screen.removed();
        screen.close();
        screen.onCurrentScreenCancelled();

        assertNull(ScrollReadStore.snapshot(), "外部切屏移除当前阅读屏也必须结算会话");
        assertEquals(List.of("{\"type\":\"scroll_read_closed\",\"v\":1}"), sentPayloads,
            "removed/close/转场回调重复到达也只能发送一条终态");
    }

    @Test
    void removed_staleScreenDoesNotSettleReplacementSession() {
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sentPayloads.add(new String(payload, java.nio.charset.StandardCharsets.UTF_8)));
        ScrollOpenViewModel oldOffer = new ScrollOpenViewModel("scroll_old", "旧卷", List.of("旧正文"));
        ScrollOpenViewModel replacement = new ScrollOpenViewModel("scroll_new", "新卷", List.of("新正文"));
        ScrollReadStore.replace(oldOffer);
        ScrollReadScreen oldScreen = new ScrollReadScreen(oldOffer);
        ScrollReadStore.replace(replacement);

        oldScreen.removed();

        assertSame(replacement, ScrollReadStore.snapshot(), "旧 screen 的 removed 不得结算后来会话");
        assertTrue(sentPayloads.isEmpty(), "旧 screen 的 removed 不得替新会话发送终态");
    }
}
