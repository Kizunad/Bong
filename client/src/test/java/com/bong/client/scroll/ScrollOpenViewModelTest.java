package com.bong.client.scroll;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * ScrollOpenViewModel 分页边界单测（plan-scroll-reading-v1 P1）。
 *
 * <p>覆盖：构造校验（空 bodyPages 拒绝——"空页拒绝"防线之二，第一道在 ScrollOpenHandler）、
 * 单页边界（1 页时 hasNextPage/hasPrevPage 恒 false）、多页边界（首页/末页/中间页）、
 * clampPageIndex 的 off-by-one（负数/越界钳边界，恰好在边界上不变）。
 */
class ScrollOpenViewModelTest {

    // ─── 构造校验："空页拒绝" ─────────────────────────────────────────────

    @Test
    void constructor_rejectsEmptyBodyPages() {
        IllegalArgumentException ex = assertThrows(
            IllegalArgumentException.class,
            () -> new ScrollOpenViewModel("scroll_x", "标题", List.of()),
            "bodyPages 为空数组应抛 IllegalArgumentException"
        );
        assertTrue(ex.getMessage().contains("body_pages"),
            "异常信息应指出是 body_pages 契约违反，实际=" + ex.getMessage());
    }

    @Test
    void constructor_rejectsNullFields() {
        assertThrows(NullPointerException.class,
            () -> new ScrollOpenViewModel(null, "标题", List.of("正文")));
        assertThrows(NullPointerException.class,
            () -> new ScrollOpenViewModel("scroll_x", null, List.of("正文")));
        assertThrows(NullPointerException.class,
            () -> new ScrollOpenViewModel("scroll_x", "标题", null));
    }

    @Test
    void constructor_copiesBodyPagesDefensively() {
        var mutable = new java.util.ArrayList<>(List.of("页1", "页2"));
        ScrollOpenViewModel vm = new ScrollOpenViewModel("scroll_x", "标题", mutable);
        mutable.add("外部追加，不应影响 vm");
        assertEquals(2, vm.bodyPages().size(),
            "构造后应防御性拷贝，外部列表变化不应影响已构造的 viewModel");
    }

    // ─── 单页边界：1 页时无上一页/下一页 ────────────────────────────────

    @Test
    void singlePage_pageCountIsOne() {
        ScrollOpenViewModel vm = new ScrollOpenViewModel("scroll_x", "标题", List.of("唯一页"));
        assertEquals(1, vm.pageCount());
    }

    @Test
    void singlePage_hasNoNextOrPrevPage() {
        ScrollOpenViewModel vm = new ScrollOpenViewModel("scroll_x", "标题", List.of("唯一页"));
        assertFalse(vm.hasNextPage(0), "单页时不应有下一页");
        assertFalse(vm.hasPrevPage(0), "单页时不应有上一页");
    }

    @Test
    void singlePage_clampAlwaysReturnsZero() {
        ScrollOpenViewModel vm = new ScrollOpenViewModel("scroll_x", "标题", List.of("唯一页"));
        assertEquals(0, vm.clampPageIndex(-5), "单页时任意负数应钳到 0");
        assertEquals(0, vm.clampPageIndex(0));
        assertEquals(0, vm.clampPageIndex(99), "单页时任意越界正数应钳到 0");
    }

    // ─── 多页边界：首页/末页/中间页 ─────────────────────────────────────

    @Test
    void multiPage_firstPage_hasNextButNoPrev() {
        ScrollOpenViewModel vm = new ScrollOpenViewModel("scroll_x", "标题",
            List.of("第一页", "第二页", "第三页"));
        assertTrue(vm.hasNextPage(0), "首页（index=0）应有下一页");
        assertFalse(vm.hasPrevPage(0), "首页（index=0）不应有上一页");
    }

    @Test
    void multiPage_lastPage_hasPrevButNoNext() {
        ScrollOpenViewModel vm = new ScrollOpenViewModel("scroll_x", "标题",
            List.of("第一页", "第二页", "第三页"));
        int last = vm.pageCount() - 1;
        assertFalse(vm.hasNextPage(last), "末页不应有下一页");
        assertTrue(vm.hasPrevPage(last), "末页应有上一页");
    }

    @Test
    void multiPage_middlePage_hasBothNextAndPrev() {
        ScrollOpenViewModel vm = new ScrollOpenViewModel("scroll_x", "标题",
            List.of("第一页", "第二页", "第三页"));
        assertTrue(vm.hasNextPage(1), "中间页（index=1）应有下一页");
        assertTrue(vm.hasPrevPage(1), "中间页（index=1）应有上一页");
    }

    // ─── clampPageIndex off-by-one ──────────────────────────────────────

    @Test
    void clampPageIndex_negativeClampsToZero() {
        ScrollOpenViewModel vm = new ScrollOpenViewModel("scroll_x", "标题",
            List.of("第一页", "第二页", "第三页"));
        assertEquals(0, vm.clampPageIndex(-1), "-1 应钳到 0");
        assertEquals(0, vm.clampPageIndex(Integer.MIN_VALUE), "极小负数应钳到 0");
    }

    @Test
    void clampPageIndex_overflowClampsToLastPage() {
        ScrollOpenViewModel vm = new ScrollOpenViewModel("scroll_x", "标题",
            List.of("第一页", "第二页", "第三页"));
        assertEquals(2, vm.clampPageIndex(3), "越界 +1（pageCount=3, index=3）应钳到末页(2)");
        assertEquals(2, vm.clampPageIndex(Integer.MAX_VALUE), "极大越界应钳到末页");
    }

    @Test
    void clampPageIndex_exactBoundaryIsUnchanged() {
        ScrollOpenViewModel vm = new ScrollOpenViewModel("scroll_x", "标题",
            List.of("第一页", "第二页", "第三页"));
        assertEquals(0, vm.clampPageIndex(0), "恰好首页边界不应变化");
        assertEquals(2, vm.clampPageIndex(2), "恰好末页边界（pageCount-1）不应变化");
    }

    @Test
    void page_returnsClampedContent() {
        ScrollOpenViewModel vm = new ScrollOpenViewModel("scroll_x", "标题",
            List.of("第一页", "第二页", "第三页"));
        assertEquals("第一页", vm.page(-1), "越界负数应回退到首页内容");
        assertEquals("第三页", vm.page(99), "越界正数应回退到末页内容");
        assertEquals("第二页", vm.page(1));
    }
}
