package com.bong.client.hud;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

/**
 * plan-tsy-search-cancel-v1 §8.1 #3 — {@link SearchHudStateStore} 回归测试。
 *
 * <p>{@code markAborted} 的 {@code AbortReason} 分支此前零覆盖（全仓无
 * {@code SearchHudStateStoreTest.java}）；本测试把 4 个真实 reason 分支 +
 * 未知字符串兜底全部 pin 死，尤其锁住 {@code cancelled} → {@code CANCELLED}
 * 这条 plan 新接的取消链路终点。
 */
class SearchHudStateStoreTest {

    @BeforeEach
    void resetStore() {
        SearchHudStateStore.resetForTests();
    }

    @AfterEach
    void cleanupStore() {
        SearchHudStateStore.resetForTests();
    }

    @Test
    void markAborted_cancelled_setsCancelledReasonAndAbortedFlashPhase() {
        SearchHudStateStore.markStarted("石匣", 100);

        SearchHudStateStore.markAborted("石匣", "cancelled");

        SearchHudState snapshot = SearchHudStateStore.snapshot();
        assertEquals(
            SearchHudState.Phase.ABORTED_FLASH,
            snapshot.phase(),
            "expected phase to flip to ABORTED_FLASH after markAborted, because HUD must show " +
                "the 1s abort flash before returning to IDLE; actual phase=" + snapshot.phase()
        );
        assertEquals(
            SearchHudState.AbortReason.CANCELLED,
            snapshot.abortReason(),
            "expected abortReason CANCELLED for reason string \"cancelled\" (this is the new " +
                "cancel_search wire terminus wired by plan-tsy-search-cancel-v1); actual=" + snapshot.abortReason()
        );
    }

    @Test
    void markAborted_moved_setsMovedReason() {
        SearchHudStateStore.markAborted("木箱", "moved");

        assertEquals(
            SearchHudState.AbortReason.MOVED,
            SearchHudStateStore.snapshot().abortReason(),
            "expected abortReason MOVED for reason string \"moved\" (SEARCH_MOVE_INTERRUPT_THRESHOLD_M " +
                "breach path); actual=" + SearchHudStateStore.snapshot().abortReason()
        );
    }

    @Test
    void markAborted_combat_setsCombatReason() {
        SearchHudStateStore.markAborted("木箱", "combat");

        assertEquals(
            SearchHudState.AbortReason.COMBAT,
            SearchHudStateStore.snapshot().abortReason(),
            "expected abortReason COMBAT for reason string \"combat\" (entering combat mid-search " +
                "interrupt path); actual=" + SearchHudStateStore.snapshot().abortReason()
        );
    }

    @Test
    void markAborted_damaged_setsDamagedReason() {
        SearchHudStateStore.markAborted("木箱", "damaged");

        assertEquals(
            SearchHudState.AbortReason.DAMAGED,
            SearchHudStateStore.snapshot().abortReason(),
            "expected abortReason DAMAGED for reason string \"damaged\" (took-hit-this-tick interrupt " +
                "path); actual=" + SearchHudStateStore.snapshot().abortReason()
        );
    }

    @Test
    void markAborted_unknownReasonString_fallsBackToNone() {
        SearchHudStateStore.markAborted("木箱", "some_future_server_reason_not_yet_known");

        assertEquals(
            SearchHudState.AbortReason.NONE,
            SearchHudStateStore.snapshot().abortReason(),
            "expected unrecognized reason strings to fall back to NONE rather than throw or " +
                "silently misclassify as an existing reason; actual=" + SearchHudStateStore.snapshot().abortReason()
        );
    }

    @Test
    void markAborted_nullReason_fallsBackToNone() {
        SearchHudStateStore.markAborted("木箱", null);

        assertEquals(
            SearchHudState.AbortReason.NONE,
            SearchHudStateStore.snapshot().abortReason(),
            "expected null reason to fall back to NONE (abortReason() treats null as empty string), " +
                "not throw NPE; actual=" + SearchHudStateStore.snapshot().abortReason()
        );
    }

    @Test
    void markAborted_blankContainerKind_fallsBackToDefaultLabel() {
        SearchHudStateStore.markAborted("  ", "cancelled");

        assertEquals(
            "容器",
            SearchHudStateStore.snapshot().containerKindZh(),
            "expected blank containerKindZh to fall back to the default \"容器\" label (safeKind " +
                "guard), same as markStarted/markProgress/markCompleted; actual=" +
                SearchHudStateStore.snapshot().containerKindZh()
        );
    }
}
