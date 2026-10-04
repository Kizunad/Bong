package com.bong.client.inventory.state;

import com.bong.client.inventory.model.InventoryItem;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** Contract pins for the declared, not-yet-wired dropped-loot projection path. */
class DroppedLootProjectionReducerTest {
    private static final DroppedLootProjectionBinding BINDING =
        new DroppedLootProjectionBinding(3L, "reset-3", "digest-3");

    @Test
    void resetAndPagesCommitOnlyAfterAllPagesWithTheSameBinding() {
        DroppedLootProjectionReducer reducer = new DroppedLootProjectionReducer();
        DroppedItemStore.Entry first = entry(1L, "first");
        DroppedItemStore.Entry second = entry(2L, "second");

        assertEquals(
            DroppedLootProjectionReducer.ApplyResult.RESET_ACCEPTED,
            reducer.applyReset(new DroppedLootProjectionReset(7L, BINDING))
        );
        assertTrue(reducer.visibleEntries().isEmpty(), "reset 必须立即清掉旧可见投影");
        assertEquals(
            DroppedLootProjectionReducer.ApplyResult.PAGE_ACCEPTED_INCOMPLETE,
            reducer.applyPage(new DroppedLootProjectionPage(7L, 0, 2, BINDING, List.of(first)))
        );
        assertTrue(reducer.visibleEntries().isEmpty(), "缺页时不能提交半个快照");
        assertEquals(
            DroppedLootProjectionReducer.ApplyResult.PROJECTION_COMMITTED,
            reducer.applyPage(new DroppedLootProjectionPage(7L, 1, 2, BINDING, List.of(second)))
        );
        assertEquals(List.of(first, second), reducer.visibleEntries());
        assertEquals(7L, reducer.highestCommittedRevision());
    }

    @Test
    void mixedRevisionOrBindingPagesFailClosedAndDoNotReplaceVisibleState() {
        DroppedLootProjectionReducer reducer = new DroppedLootProjectionReducer();
        DroppedLootProjectionBinding otherBinding =
            new DroppedLootProjectionBinding(3L, "reset-other", "digest-other");
        DroppedItemStore.Entry oldEntry = entry(3L, "old");

        reducer.applyReset(new DroppedLootProjectionReset(9L, BINDING));
        reducer.applyPage(new DroppedLootProjectionPage(9L, 0, 1, BINDING, List.of(oldEntry)));
        assertEquals(List.of(oldEntry), reducer.visibleEntries());

        assertEquals(
            DroppedLootProjectionReducer.ApplyResult.REJECTED,
            reducer.applyPage(new DroppedLootProjectionPage(10L, 0, 1, otherBinding, List.of()))
        );
        assertEquals(List.of(oldEntry), reducer.visibleEntries());
        assertEquals(
            DroppedLootProjectionReducer.ApplyResult.IGNORED_STALE,
            reducer.applyReset(new DroppedLootProjectionReset(9L, BINDING))
        );
        assertEquals(List.of(oldEntry), reducer.visibleEntries());
    }

    @Test
    void emptyProjectionIsACommittedSingleEmptyPageAndClearsOldEntries() {
        DroppedLootProjectionReducer reducer = new DroppedLootProjectionReducer();
        DroppedItemStore.Entry oldEntry = entry(4L, "old");
        reducer.applyReset(new DroppedLootProjectionReset(11L, BINDING));
        reducer.applyPage(new DroppedLootProjectionPage(11L, 0, 1, BINDING, List.of(oldEntry)));
        assertFalse(reducer.visibleEntries().isEmpty());

        DroppedLootProjectionBinding nextBinding =
            new DroppedLootProjectionBinding(3L, "reset-12", "digest-12");
        reducer.applyReset(new DroppedLootProjectionReset(12L, nextBinding));
        assertEquals(
            DroppedLootProjectionReducer.ApplyResult.PROJECTION_COMMITTED,
            reducer.applyPage(new DroppedLootProjectionPage(12L, 0, 1, nextBinding, List.of()))
        );
        assertTrue(reducer.visibleEntries().isEmpty());
    }

    @Test
    void completeSnapshotMayAdvanceWithoutResetButCannotChangeContextBinding() {
        DroppedLootProjectionReducer reducer = new DroppedLootProjectionReducer();
        DroppedItemStore.Entry entry = entry(5L, "first");
        assertEquals(
            DroppedLootProjectionReducer.ApplyResult.PROJECTION_COMMITTED,
            reducer.applyPage(new DroppedLootProjectionPage(1L, 0, 1, BINDING, List.of(entry)))
        );
        assertEquals(
            DroppedLootProjectionReducer.ApplyResult.IGNORED_STALE,
            reducer.applyPage(new DroppedLootProjectionPage(1L, 0, 1, BINDING, List.of()))
        );
        assertEquals(List.of(entry), reducer.visibleEntries());
    }

    @Test
    void assemblerRejectsConflictingPagesAndKeepsTheFirstPage() {
        DroppedItemStore.Entry first = entry(6L, "first");
        DroppedItemStore.Entry conflicting = entry(7L, "conflicting");
        DroppedLootProjectionPage firstPage =
            new DroppedLootProjectionPage(20L, 0, 2, BINDING, List.of(first));
        DroppedLootPageAssembler assembler = new DroppedLootPageAssembler(firstPage);

        assertEquals(
            DroppedLootPageAssembler.AddResult.REJECTED,
            assembler.add(new DroppedLootProjectionPage(20L, 0, 2, BINDING, List.of(conflicting)))
        );
        assertEquals(
            DroppedLootPageAssembler.AddResult.ACCEPTED_COMPLETE,
            assembler.add(new DroppedLootProjectionPage(20L, 1, 2, BINDING, List.of()))
        );
        assertEquals(List.of(first), assembler.entries());
    }

    private static DroppedItemStore.Entry entry(long instanceId, String itemId) {
        return new DroppedItemStore.Entry(
            instanceId,
            "main_pack",
            0,
            0,
            1.0,
            64.0,
            1.0,
            InventoryItem.simple(itemId, itemId)
        );
    }
}
