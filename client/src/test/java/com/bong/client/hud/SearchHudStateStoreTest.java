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
    private static final long ONE_SECOND_NANOS = 1_000_000_000L;
    private static final long THREE_SECONDS_NANOS = 3_000_000_000L;

    @BeforeEach
    void resetStore() {
        SearchHudStateStore.resetForTests();
    }

    @AfterEach
    void cleanupStore() {
        SearchHudStateStore.resetForTests();
    }

    @Test
    void completedFlash_expiresExactlyAtThreeSecondBoundary() {
        long startedAt = 10_000L;
        SearchHudStateStore.markCompletedAtNanos("石匣", startedAt);

        SearchHudState beforeBoundary = SearchHudStateStore.snapshotAtNanos(
            startedAt + THREE_SECONDS_NANOS - 1L
        );
        assertEquals(
            SearchHudState.Phase.COMPLETED_FLASH,
            beforeBoundary.phase(),
            "completed flash 必须完整保留 3 秒；边界前 1ns 不得提前消失，actual=" + beforeBoundary.phase()
        );

        SearchHudState atBoundary = SearchHudStateStore.snapshotAtNanos(startedAt + THREE_SECONDS_NANOS);
        assertEquals(
            SearchHudState.Phase.IDLE,
            atBoundary.phase(),
            "completed flash 到达 3 秒精确边界时必须回 IDLE，不能永久污染同一 session HUD，actual=" +
                atBoundary.phase()
        );
    }

    @Test
    void abortedFlash_expiresExactlyAtOneSecondBoundary() {
        long startedAt = 20_000L;
        SearchHudStateStore.markAbortedAtNanos("储物袋残骸", "moved", startedAt);

        SearchHudState beforeBoundary = SearchHudStateStore.snapshotAtNanos(startedAt + ONE_SECOND_NANOS - 1L);
        assertEquals(
            SearchHudState.Phase.ABORTED_FLASH,
            beforeBoundary.phase(),
            "aborted flash 必须完整保留 1 秒；边界前 1ns 不得提前消失，actual=" + beforeBoundary.phase()
        );

        SearchHudState atBoundary = SearchHudStateStore.snapshotAtNanos(startedAt + ONE_SECOND_NANOS);
        assertEquals(
            SearchHudState.Phase.IDLE,
            atBoundary.phase(),
            "aborted flash 到达 1 秒精确边界时必须回 IDLE，actual=" + atBoundary.phase()
        );
    }

    @Test
    void newSearch_overridesOldTerminalFlashTimer() {
        SearchHudStateStore.markCompletedAtNanos("石匣", 30_000L);

        SearchHudStateStore.markStarted("骨匣", 80);

        SearchHudState state = SearchHudStateStore.snapshotAtNanos(THREE_SECONDS_NANOS + 30_000L);
        assertEquals(
            SearchHudState.Phase.SEARCHING,
            state.phase(),
            "新一轮 search_started 必须覆盖旧 completed flash 的计时器，不能在旧 deadline 到点时误清新搜索"
        );
        assertEquals("骨匣", state.containerKindZh());
        assertEquals(80, state.requiredTicks());
    }

    @Test
    void progressUpdate_overridesOldTerminalFlashTimer() {
        SearchHudStateStore.markAbortedAtNanos("石匣", "combat", 40_000L);

        SearchHudStateStore.markProgress("骨匣", 20, 100);

        SearchHudState state = SearchHudStateStore.snapshotAtNanos(ONE_SECOND_NANOS + 40_000L);
        assertEquals(
            SearchHudState.Phase.SEARCHING,
            state.phase(),
            "新 search_progress 必须覆盖旧 aborted flash 的计时器，不能误清正在进行的搜索"
        );
        assertEquals(20, state.elapsedTicks());
        assertEquals(100, state.requiredTicks());
    }

    @Test
    void newerTerminalFlash_replacesPreviousPhaseAndDeadline() {
        long completedAt = 50_000L;
        long abortedAt = completedAt + 2_500_000_000L;
        SearchHudStateStore.markCompletedAtNanos("旧石匣", completedAt);

        SearchHudStateStore.markAbortedAtNanos("新骨匣", "damaged", abortedAt);

        SearchHudState beforeNewDeadline = SearchHudStateStore.snapshotAtNanos(abortedAt + ONE_SECOND_NANOS - 1L);
        assertEquals(SearchHudState.Phase.ABORTED_FLASH, beforeNewDeadline.phase());
        assertEquals("新骨匣", beforeNewDeadline.containerKindZh());
        assertEquals(SearchHudState.AbortReason.DAMAGED, beforeNewDeadline.abortReason());
        assertEquals(
            SearchHudState.Phase.IDLE,
            SearchHudStateStore.snapshotAtNanos(abortedAt + ONE_SECOND_NANOS).phase(),
            "后到的 aborted flash 必须使用自己的 1 秒 deadline，不能沿用旧 completed flash 的 3 秒 deadline"
        );
    }

    @Test
    void monotonicClockRollback_doesNotPrematurelyExpireFlash() {
        SearchHudStateStore.markCompletedAtNanos("石匣", 60_000L);

        SearchHudState state = SearchHudStateStore.snapshotAtNanos(59_999L);

        assertEquals(
            SearchHudState.Phase.COMPLETED_FLASH,
            state.phase(),
            "测试时钟出现负向时间差时不得把 flash 误判为已过期，actual=" + state.phase()
        );
    }

    @Test
    void nanoTimeWrap_preservesShortElapsedDurationSemantics() {
        long startedAt = Long.MAX_VALUE - 500_000_000L;
        SearchHudStateStore.markAbortedAtNanos("石匣", "cancelled", startedAt);

        assertEquals(
            SearchHudState.Phase.ABORTED_FLASH,
            SearchHudStateStore.snapshotAtNanos(startedAt + ONE_SECOND_NANOS - 1L).phase(),
            "System.nanoTime 长整型回绕后，边界前的短时长差仍应保持 aborted flash"
        );
        assertEquals(
            SearchHudState.Phase.IDLE,
            SearchHudStateStore.snapshotAtNanos(startedAt + ONE_SECOND_NANOS).phase(),
            "System.nanoTime 长整型回绕后，1 秒精确边界仍必须回 IDLE"
        );
    }

    @Test
    void clearOnDisconnect_clearsSearchingAndTerminalStates() {
        SearchHudStateStore.markStarted("石匣", 100);
        SearchHudStateStore.clearOnDisconnect();
        assertEquals(
            SearchHudState.Phase.IDLE,
            SearchHudStateStore.snapshotAtNanos(70_000L).phase(),
            "断线必须立即清空进行中的搜刮 HUD，避免 reconnect 后沿用旧 session 状态"
        );

        SearchHudStateStore.markAbortedAtNanos("石匣", "combat", 80_000L);
        SearchHudStateStore.clearOnDisconnect();
        assertEquals(
            SearchHudState.Phase.IDLE,
            SearchHudStateStore.snapshotAtNanos(80_001L).phase(),
            "断线必须立即清空终态 flash，避免 reconnect 后残留旧 session 提示"
        );
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
