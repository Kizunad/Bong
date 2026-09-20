package com.bong.client.fauna;

import org.junit.jupiter.api.Test;

import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class FaunaPreviewCommandTest {
    @Test
    void at_capacity_discards_oldest_preview_before_new_one_is_added() {
        List<String> previews = new ArrayList<>(List.of("oldest", "newest"));
        List<String> discarded = new ArrayList<>();

        FaunaPreviewCommand.evictOldestIfAtCapacity(previews, 2, discarded::add);
        previews.add("new");

        assertEquals(
            List.of("newest", "new"),
            previews,
            "达到预览上限时必须先移除最旧实体，才能把新预览保持在有界列表内"
        );
        assertEquals(
            List.of("oldest"),
            discarded,
            "被淘汰的最旧实体必须调用 discard，不能只从 bookkeeping 列表删除"
        );
    }

    @Test
    void below_capacity_keeps_existing_previews_and_does_not_discard() {
        List<String> previews = new ArrayList<>(List.of("only"));
        List<String> discarded = new ArrayList<>();

        FaunaPreviewCommand.evictOldestIfAtCapacity(previews, 2, discarded::add);

        assertEquals(List.of("only"), previews, "未达到上限时不能淘汰现有预览");
        assertEquals(List.of(), discarded, "未达到上限时不得调用 discard");
    }

    @Test
    void disconnect_cleanup_discards_every_preview_before_clearing_the_list() {
        List<String> previews = new ArrayList<>(List.of("first", "second", "third"));
        List<String> discarded = new ArrayList<>();

        FaunaPreviewCommand.discardAndClear(previews, discarded::add);

        assertEquals(
            List.of("first", "second", "third"),
            discarded,
            "断线清理必须让此前的每个预览实体都进入 discard 状态，不能只丢 bookkeeping 引用"
        );
        assertEquals(List.of(), previews, "断线清理完成后不得保留已 discard 预览的引用");
    }

    @Test
    void discard_failure_still_attempts_all_previews_clears_list_and_suppresses_later_failures() {
        List<String> previews = new ArrayList<>(List.of("first", "second", "third"));
        List<String> attempted = new ArrayList<>();
        RuntimeException firstFailure = new IllegalStateException("first discard failed");
        RuntimeException laterFailure = new IllegalArgumentException("later discard failed");

        RuntimeException thrown = assertThrows(
            RuntimeException.class,
            () -> FaunaPreviewCommand.discardAndClear(previews, preview -> {
                attempted.add(preview);
                if (preview.equals("first")) throw firstFailure;
                if (preview.equals("second")) throw laterFailure;
            })
        );

        assertSame(firstFailure, thrown, "必须重抛第一个 discard 异常，保留最早失败原因");
        assertEquals(
            List.of("first", "second", "third"),
            attempted,
            "某个实体 discard 失败后仍必须尝试列表中的每个预览实体"
        );
        assertTrue(previews.isEmpty(), "即使 discard 失败，清理职责也必须最终清空预览列表");
        assertEquals(1, thrown.getSuppressed().length, "后续 discard 异常必须保留为一个 suppressed 原因");
        assertSame(laterFailure, thrown.getSuppressed()[0], "suppressed 必须是后续实体的实际异常");
    }

    @Test
    void disconnect_cleanup_resets_session_state_even_when_discard_fails() throws Exception {
        Field previewsField = field("PREVIEWS");
        @SuppressWarnings("unchecked")
        List<FaunaEntity> previews = (List<FaunaEntity>) previewsField.get(null);
        Field selectedField = field("selected");
        Field nextIdField = field("nextId");
        sun.misc.Unsafe unsafe = unsafe();
        Object selectedBase = unsafe.staticFieldBase(selectedField);
        long selectedOffset = unsafe.staticFieldOffset(selectedField);

        previews.clear();
        previews.add(null);
        unsafe.putObject(selectedBase, selectedOffset, new Object());
        nextIdField.setInt(null, 12345);
        try {
            assertThrows(
                NullPointerException.class,
                FaunaPreviewCommand::clearOnDisconnect,
                "discard 失败仍必须向调用方报告清理失败"
            );
            assertNull(selectedField.get(null), "清理失败时也必须丢弃上一会话的 selected 引用");
            assertEquals(-300_000, nextIdField.getInt(null), "清理失败时也必须重置预览实体 ID 游标");
        } finally {
            previews.clear();
            unsafe.putObject(selectedBase, selectedOffset, null);
            nextIdField.setInt(null, -300_000);
        }
    }

    private static Field field(String name) throws ReflectiveOperationException {
        Field field = FaunaPreviewCommand.class.getDeclaredField(name);
        field.setAccessible(true);
        return field;
    }

    private static sun.misc.Unsafe unsafe() throws ReflectiveOperationException {
        Field field = sun.misc.Unsafe.class.getDeclaredField("theUnsafe");
        field.setAccessible(true);
        return (sun.misc.Unsafe) field.get(null);
    }
}
