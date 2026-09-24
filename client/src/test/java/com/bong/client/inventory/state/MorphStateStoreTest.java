package com.bong.client.inventory.state;

import com.bong.client.inventory.model.MorphEntry;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.Map;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-race-system-v1 PR-5b — {@link MorphStateStore} 直接单测（仿
 * {@code RaceGateMetaStore} 的 volatile + listener 惯例，但 key 是 per-entity）。
 *
 * <p>覆盖：① applyFull 整表替换 + null 视空表 ② applyDelta 插入/移除/覆盖 ③ 查表 miss
 * 语义（未易形） ④ listener 通知（full 与 delta 各触发一次）⑤ resetForTests 幂等。
 */
class MorphStateStoreTest {

    @BeforeEach
    void setUp() { MorphStateStore.resetForTests(); }

    @AfterEach
    void tearDown() { MorphStateStore.resetForTests(); }

    @Test
    void morphOfMissesByDefault() {
        assertTrue(MorphStateStore.morphOf(1).isEmpty(), "未收到任何 payload 时查表应 miss");
    }

    @Test
    void applyFullReplacesTableAndNullTreatedAsEmpty() {
        MorphStateStore.applyFull(Map.of(1, new MorphEntry(0, "whale", "whale")));
        assertTrue(MorphStateStore.morphOf(1).isPresent());

        MorphStateStore.applyFull(null);
        assertTrue(MorphStateStore.morphOf(1).isEmpty(), "null 应视为空表，清空旧数据");
    }

    @Test
    void applyFullOverwritesPreviousFullSnapshot() {
        MorphStateStore.applyFull(Map.of(1, new MorphEntry(0, "whale", "whale")));
        MorphStateStore.applyFull(Map.of(2, new MorphEntry(0, "whale", "whale")));

        assertTrue(MorphStateStore.morphOf(1).isEmpty(), "新 full 快照应整表替换，旧条目应消失");
        assertTrue(MorphStateStore.morphOf(2).isPresent());
    }

    @Test
    void applyDeltaWithNonNullEntryInsertsOrUpdates() {
        MorphStateStore.applyDelta(5, new MorphEntry(0, "whale", "whale"));
        assertTrue(MorphStateStore.morphOf(5).isPresent());
        assertEquals("whale", MorphStateStore.morphOf(5).get().formRaceId());

        // 覆盖更新（同 entity_id 再来一条）。
        MorphStateStore.applyDelta(5, new MorphEntry(1, "whale", "whale"));
        assertEquals(1, MorphStateStore.morphOf(5).get().modelKind());
    }

    @Test
    void applyDeltaWithNullEntryRemoves() {
        MorphStateStore.applyDelta(5, new MorphEntry(0, "whale", "whale"));
        assertTrue(MorphStateStore.morphOf(5).isPresent());

        MorphStateStore.applyDelta(5, null);
        assertTrue(MorphStateStore.morphOf(5).isEmpty(), "entry=null 应移除该 entity_id");
    }

    @Test
    void applyDeltaRemovingUnknownEntityIdIsNoopNotCrash() {
        MorphStateStore.applyDelta(999, null);
        assertTrue(MorphStateStore.morphOf(999).isEmpty());
    }

    @Test
    void applyDeltaDoesNotAffectOtherEntities() {
        MorphStateStore.applyFull(Map.of(
            1, new MorphEntry(0, "whale", "whale"),
            2, new MorphEntry(0, "whale", "whale")
        ));
        MorphStateStore.applyDelta(1, null);

        assertTrue(MorphStateStore.morphOf(1).isEmpty());
        assertTrue(MorphStateStore.morphOf(2).isPresent(), "delta 移除一个 entity 不应影响其余条目");
    }

    @Test
    void listenersNotifiedOnFullAndDelta() {
        AtomicInteger calls = new AtomicInteger(0);
        Runnable listener = calls::incrementAndGet;
        MorphStateStore.addListener(listener);
        try {
            MorphStateStore.applyFull(Map.of(1, new MorphEntry(0, "whale", "whale")));
            assertEquals(1, calls.get());
            MorphStateStore.applyDelta(2, new MorphEntry(0, "whale", "whale"));
            assertEquals(2, calls.get());
            MorphStateStore.applyDelta(2, null);
            assertEquals(3, calls.get());
        } finally {
            MorphStateStore.removeListener(listener);
        }
    }

    @Test
    void removedListenerNoLongerReceivesNotifications() {
        AtomicInteger calls = new AtomicInteger(0);
        Runnable listener = calls::incrementAndGet;
        MorphStateStore.addListener(listener);
        MorphStateStore.removeListener(listener);

        MorphStateStore.applyFull(Map.of(1, new MorphEntry(0, "whale", "whale")));
        assertFalse(calls.get() > 0, "移除的 listener 不应再被通知");
    }
}
