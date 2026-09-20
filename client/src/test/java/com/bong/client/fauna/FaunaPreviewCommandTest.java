package com.bong.client.fauna;

import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;

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
}
