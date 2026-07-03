package com.bong.client.scroll;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
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
}
