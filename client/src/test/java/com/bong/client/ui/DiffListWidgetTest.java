package com.bong.client.ui;

import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Component;
import io.wispforest.owo.ui.core.Sizing;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.BiConsumer;
import java.util.function.Function;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class DiffListWidgetTest {
    private record Item(int key, String value) {}

    private static FlowLayout rows() {
        return Containers.verticalFlow(Sizing.content(), Sizing.content());
    }

    private static Item item(int key) {
        return new Item(key, "item-" + key);
    }

    private static DiffListWidget<Item, Integer, FlowLayout> widget(
        FlowLayout rows,
        AtomicInteger created,
        List<Integer> patched
    ) {
        Function<Item, Integer> keyOf = Item::key;
        Function<Item, FlowLayout> createRow = item -> {
            created.incrementAndGet();
            return Containers.horizontalFlow(Sizing.content(), Sizing.content());
        };
        BiConsumer<FlowLayout, Item> patchRow = (row, item) -> patched.add(item.key());
        return new DiffListWidget<>(rows, keyOf, createRow, patchRow);
    }

    @Test
    void emptyAndItemsRebuildThenSameOrderedKeysPatch() {
        FlowLayout rows = rows();
        AtomicInteger created = new AtomicInteger();
        List<Integer> patched = new ArrayList<>();
        DiffListWidget<Item, Integer, FlowLayout> widget = widget(rows, created, patched);

        assertEquals(DiffListWidget.UpdateResult.PATCHED, widget.update(List.of()),
            "空的初始序列没有结构变化，因此应走 PATCHED");
        assertEquals(DiffListWidget.UpdateResult.REBUILT, widget.update(List.of(item(1), item(2))),
            "从空序列到两行是结构变化，必须整体重建");
        List<Component> before = List.copyOf(rows.children());
        assertEquals(DiffListWidget.UpdateResult.PATCHED, widget.update(List.of(
            new Item(1, "updated"), new Item(2, "updated")
        )), "有序 key 未变时只 patch");
        assertEquals(List.of(1, 2), patched, "patch 必须按有序 key 逐行执行");
        assertEquals(before, rows.children(), "patch 不得替换已挂载行");
        assertEquals(2, created.get(), "相同有序 key 不应重复创建行");
    }

    @Test
    void reorderAddAndRemoveRebuildWithNewOrderedRows() {
        FlowLayout rows = rows();
        DiffListWidget<Item, Integer, FlowLayout> widget = widget(rows, new AtomicInteger(), new ArrayList<>());
        widget.update(List.of(item(1), item(2), item(3)));
        FlowLayout oldOne = (FlowLayout) rows.children().get(0);

        assertEquals(DiffListWidget.UpdateResult.REBUILT, widget.update(List.of(item(3), item(1))),
            "重排并删除必须整体重建");
        assertEquals(List.of(3, 1), widget.renderedKeys());
        assertEquals(2, rows.children().size());
        assertFalse(rows.children().contains(oldOne), "结构重建必须替换旧 children");
    }

    @Test
    void invalidInputIsRejectedBeforeAnyMutation() {
        FlowLayout rows = rows();
        AtomicInteger created = new AtomicInteger();
        DiffListWidget<Item, Integer, FlowLayout> widget = widget(rows, created, new ArrayList<>());
        widget.update(List.of(item(1)));
        List<Component> before = List.copyOf(rows.children());
        List<Integer> keysBefore = widget.renderedKeys();

        assertThrows(NullPointerException.class, () -> widget.update(null));
        assertThrows(IllegalArgumentException.class, () -> widget.update(java.util.Arrays.asList(item(1), null)));
        assertThrows(IllegalArgumentException.class, () -> widget.update(List.of(new Item(0, "a"), new Item(0, "b"))));
        DiffListWidget<Item, Integer, FlowLayout> nullKeyWidget = new DiffListWidget<>(
            rows(), ignored -> null, ignored -> Containers.horizontalFlow(Sizing.content(), Sizing.content()), (r, i) -> {}
        );
        assertThrows(IllegalArgumentException.class, () -> nullKeyWidget.update(List.of(new Item(0, "null"))));

        assertEquals(keysBefore, widget.renderedKeys(), "非法输入必须保留已提交 key 序列");
        assertEquals(before, rows.children(), "非法输入必须在 mutation 前失败");
        assertEquals(1, created.get(), "非法输入不应触发 createRow");
    }

    @Test
    void patchFailurePreservesCommittedSequenceAndRetriesFromTheBeginning() {
        FlowLayout rows = rows();
        List<Integer> patched = new ArrayList<>();
        AtomicInteger attempts = new AtomicInteger();
        DiffListWidget<Item, Integer, FlowLayout> widget = new DiffListWidget<>(
            rows,
            Item::key,
            ignored -> Containers.horizontalFlow(Sizing.content(), Sizing.content()),
            (row, item) -> {
                patched.add(item.key());
                if (attempts.getAndIncrement() == 1) {
                    throw new IllegalStateException("patch failed");
                }
            }
        );
        widget.update(List.of(item(1), item(2)));
        List<Component> before = List.copyOf(rows.children());

        assertThrows(IllegalStateException.class, () -> widget.update(List.of(new Item(1, "a"), new Item(2, "b"))));
        assertEquals(List.of(1, 2), widget.renderedKeys(), "patch 失败不能提交新序列");
        assertEquals(before, rows.children(), "patch 失败不能替换 mounted rows");

        patched.clear();
        assertEquals(DiffListWidget.UpdateResult.PATCHED,
            widget.update(List.of(new Item(1, "retry"), new Item(2, "retry"))));
        assertEquals(List.of(1, 2), patched, "重试必须从完整列表首行开始");
    }

    @Test
    void createFailureLeavesOldRowsAndCommittedKeysUntouched() {
        FlowLayout rows = rows();
        AtomicInteger creates = new AtomicInteger();
        DiffListWidget<Item, Integer, FlowLayout> widget = new DiffListWidget<>(
            rows,
            Item::key,
            item -> {
                if (creates.incrementAndGet() == 2) throw new IllegalStateException("create failed");
                return Containers.horizontalFlow(Sizing.content(), Sizing.content());
            },
            (row, item) -> {}
        );
        widget.update(List.of(item(1)));
        List<Component> before = List.copyOf(rows.children());

        assertThrows(IllegalStateException.class, () -> widget.update(List.of(item(1), item(2))));
        assertEquals(List.of(1), widget.renderedKeys());
        assertEquals(before, rows.children(), "createRow 失败发生在 host mutation 前");
    }

    @Test
    void inspectionIsImmutableAndLookupIsOptional() {
        FlowLayout rows = rows();
        DiffListWidget<Item, Integer, FlowLayout> widget = widget(rows, new AtomicInteger(), new ArrayList<>());
        widget.update(List.of(item(7)));

        List<Integer> keys = widget.renderedKeys();
        assertThrows(UnsupportedOperationException.class, () -> keys.add(8));
        assertTrue(widget.rowForKey(7).isPresent());
        assertSame(rows.children().get(0), widget.rowForKey(7).orElseThrow());
        assertEquals(Optional.empty(), widget.rowForKey(8));
    }
}
