package com.bong.client.craft;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * fix/craft-recipe-list-scroll-bounce — 锁住 {@link CraftRecipeListWidget#needsRebuild} 的判定契约。
 *
 * <p>owo-lib 0.11.2 的 ScrollContainer.layout() 会在内容清空的瞬间把 maxScroll 算成 0 并把
 * scrollOffset clamp 回 0；CraftRecipeListWidget.refresh() 每次都 rows.clearChildren() 全量重建
 * 会触发这个 clamp，表现为"滚动条自动回弹到顶部"。needsRebuild() 是 diff 式 refresh 的核心判定：
 * id 序列（含顺序）完全一致就不重建 —— 这个纯函数抽出来是因为 owo UI 组件本身在纯 JUnit
 * 里起不来 MC，没法端到端测滚动条位置，只能把"是否需要重建"这个决定性判断单独锁死。
 */
class CraftRecipeListWidgetTest {

    @Test
    void identicalSequenceDoesNotNeedRebuild() {
        List<String> current = List.of("a", "b", "c");
        List<String> next = List.of("a", "b", "c");
        assertFalse(CraftRecipeListWidget.needsRebuild(current, next),
            "期望 false 因为 id 序列内容与顺序完全一致（不同 List 实例但 equals），"
                + "实际 " + CraftRecipeListWidget.needsRebuild(current, next));
    }

    @Test
    void sameInstanceDoesNotNeedRebuild() {
        List<String> ids = List.of("a", "b", "c");
        assertFalse(CraftRecipeListWidget.needsRebuild(ids, ids),
            "期望 false 因为 current 与 next 是同一引用，退化为 identity 情形，"
                + "实际 " + CraftRecipeListWidget.needsRebuild(ids, ids));
    }

    @Test
    void reorderedSequenceNeedsRebuild() {
        List<String> current = List.of("a", "b", "c");
        List<String> next = List.of("c", "b", "a");
        assertTrue(CraftRecipeListWidget.needsRebuild(current, next),
            "期望 true 因为收藏置顶/排序变化会让相同 id 集合换了顺序，"
                + "行的视觉位置必须跟着换 —— 原地更新做不到重排，实际 "
                + CraftRecipeListWidget.needsRebuild(current, next));
    }

    @Test
    void addedIdNeedsRebuild() {
        List<String> current = List.of("a", "b");
        List<String> next = List.of("a", "b", "c");
        assertTrue(CraftRecipeListWidget.needsRebuild(current, next),
            "期望 true 因为新增了一个配方 id（如解锁新配方），需要新建一行组件，"
                + "实际 " + CraftRecipeListWidget.needsRebuild(current, next));
    }

    @Test
    void removedIdNeedsRebuild() {
        List<String> current = List.of("a", "b", "c");
        List<String> next = List.of("a", "b");
        assertTrue(CraftRecipeListWidget.needsRebuild(current, next),
            "期望 true 因为切分类/收紧搜索导致某个 id 被过滤掉，多出来的行组件必须移除，"
                + "实际 " + CraftRecipeListWidget.needsRebuild(current, next));
    }

    @Test
    void emptyToNonEmptyNeedsRebuild() {
        List<String> current = List.of();
        List<String> next = List.of("a");
        assertTrue(CraftRecipeListWidget.needsRebuild(current, next),
            "期望 true 因为空列表下渲染的是「无匹配配方」占位 label，"
                + "转为有配方时必须换掉占位组件，实际 " + CraftRecipeListWidget.needsRebuild(current, next));
    }

    @Test
    void nonEmptyToEmptyNeedsRebuild() {
        List<String> current = List.of("a");
        List<String> next = List.of();
        assertTrue(CraftRecipeListWidget.needsRebuild(current, next),
            "期望 true 因为搜索/切分类后配方行清空，必须换成「无匹配配方」占位 label，"
                + "实际 " + CraftRecipeListWidget.needsRebuild(current, next));
    }

    @Test
    void bothEmptyDoesNotNeedRebuild() {
        List<String> current = List.of();
        List<String> next = List.of();
        assertFalse(CraftRecipeListWidget.needsRebuild(current, next),
            "期望 false 因为两次都是空配方列表（同样展示「无匹配配方」占位），无需重建，"
                + "实际 " + CraftRecipeListWidget.needsRebuild(current, next));
    }

    @Test
    void singleElementSameIdDoesNotNeedRebuild() {
        List<String> current = List.of("only");
        List<String> next = List.of("only");
        assertFalse(CraftRecipeListWidget.needsRebuild(current, next),
            "期望 false 因为唯一一行的 id 未变（如仅数量/收藏态变化触发的 refresh），"
                + "实际 " + CraftRecipeListWidget.needsRebuild(current, next));
    }

    @Test
    void differentLengthWithSharedPrefixNeedsRebuild() {
        List<String> current = List.of("a", "b", "c");
        List<String> next = List.of("a", "b");
        assertTrue(CraftRecipeListWidget.needsRebuild(current, next),
            "期望 true 因为即便前缀完全相同，长度不同意味着行数不同，必须重建，"
                + "实际 " + CraftRecipeListWidget.needsRebuild(current, next));
    }

    // ---- shouldRebuildRows：refresh() 实际使用的完整判定（rowsBuilt 首刷边界 + id 序列 diff） ----

    @Test
    void firstRefreshWithEmptyListMustRebuild() {
        assertTrue(CraftRecipeListWidget.shouldRebuildRows(false, List.of(), List.of()),
            "期望 true 因为首次 refresh（rowsBuilt=false）必须走重建路径渲染「无匹配配方」占位，"
                + "即使 id 序列与初值同为空 —— 否则占位 label 永远不出现，"
                + "实际 " + CraftRecipeListWidget.shouldRebuildRows(false, List.of(), List.of()));
    }

    @Test
    void firstRefreshWithRecipesMustRebuild() {
        assertTrue(CraftRecipeListWidget.shouldRebuildRows(false, List.of(), List.of("a", "b")),
            "期望 true 因为首次 refresh（rowsBuilt=false）没有任何已渲染行，必须重建，"
                + "实际 " + CraftRecipeListWidget.shouldRebuildRows(false, List.of(), List.of("a", "b")));
    }

    @Test
    void builtAndIdenticalSequenceSkipsRebuild() {
        assertFalse(CraftRecipeListWidget.shouldRebuildRows(true, List.of("a", "b"), List.of("a", "b")),
            "期望 false 因为已完成首刷（rowsBuilt=true）且 id 序列完全一致，"
                + "应走原地更新路径以保留滚动位置，"
                + "实际 " + CraftRecipeListWidget.shouldRebuildRows(true, List.of("a", "b"), List.of("a", "b")));
    }

    @Test
    void builtButSequenceChangedMustRebuild() {
        assertTrue(CraftRecipeListWidget.shouldRebuildRows(true, List.of("a", "b"), List.of("b", "a")),
            "期望 true 因为已完成首刷但 id 序列发生重排，行的视觉位置必须跟着换，"
                + "实际 " + CraftRecipeListWidget.shouldRebuildRows(true, List.of("a", "b"), List.of("b", "a")));
    }

    @Test
    void builtAndBothEmptySkipsRebuild() {
        assertFalse(CraftRecipeListWidget.shouldRebuildRows(true, List.of(), List.of()),
            "期望 false 因为首刷已渲染过「无匹配配方」占位（rowsBuilt=true）且仍为空列表，"
                + "占位无需重建，实际 " + CraftRecipeListWidget.shouldRebuildRows(true, List.of(), List.of()));
    }
}
