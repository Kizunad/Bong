package com.bong.client.craft;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 锁定 CraftScreen / WorkbenchScreen 共用的玩家可感知反馈契约：
 * completed → flashTicks=6 → 恰好一声完成音 → refresh；
 * failed → 无完成音、仍 refresh；listener 注销后不再触发。
 */
class CraftOutcomeFeedbackTest {
    @BeforeEach
    void setUp() {
        CraftStore.clearAllListenersForTests();
        CraftStore.clear();
    }

    @AfterEach
    void tearDown() {
        CraftStore.clearAllListenersForTests();
        CraftStore.clear();
    }

    @Test
    void completedSetsFlashTicksPlaysOneSoundThenRefreshInOrder() {
        List<String> events = new ArrayList<>();
        AtomicInteger flash = new AtomicInteger(-1);

        CraftOutcomeFeedback.apply(
            CraftStore.CraftOutcomeEvent.completed("craft.a", "out", 1, 10L),
            ticks -> {
                flash.set(ticks);
                events.add("flash=" + ticks);
            },
            () -> events.add("sound"),
            () -> events.add("refresh")
        );

        assertEquals(CraftOutcomeFeedback.COMPLETED_FLASH_TICKS, flash.get(),
            "completed 必须写 flashTicks=6（与历史 CraftScreen/WorkbenchScreen 一致）");
        assertEquals(List.of("flash=6", "sound", "refresh"), events,
            "completed 顺序必须是 flash → 完成音 → refresh，且各恰好一次；实际=" + events);
    }

    @Test
    void failedDoesNotPlayCompleteSoundButStillRefreshes() {
        List<String> events = new ArrayList<>();
        AtomicInteger flash = new AtomicInteger(-1);

        CraftOutcomeFeedback.apply(
            CraftStore.CraftOutcomeEvent.failed("craft.b", "player_cancelled", 2, 0.0),
            ticks -> {
                flash.set(ticks);
                events.add("flash=" + ticks);
            },
            () -> events.add("sound"),
            () -> events.add("refresh")
        );

        assertEquals(-1, flash.get(), "failed 不得写 flashTicks；实际=" + flash.get());
        assertEquals(List.of("refresh"), events,
            "failed 只应 refresh，且不得播放完成音；实际=" + events);
    }

    @Test
    void outcomeViewProjectsBothStoreVariantsWithoutLeakingEventType() {
        CraftStore.CraftOutcomeEvent completed = CraftStore.CraftOutcomeEvent.completed(
            "craft.view.completed", "rough_handle", 2, 12L
        );
        CraftStore.CraftOutcomeEvent failed = CraftStore.CraftOutcomeEvent.failed(
            "craft.view.failed", "insufficient_material", 3, 0.25
        );

        CraftOutcomeView completedView = CraftOutcomeView.from(completed);
        CraftOutcomeView failedView = CraftOutcomeView.from(failed);

        assertEquals(CraftOutcomeView.Kind.COMPLETED, completedView.kind());
        assertEquals("rough_handle", completedView.outputTemplate());
        assertEquals(2, completedView.outputCount());
        assertEquals(12L, completedView.completedAtTick());
        assertEquals(CraftOutcomeView.Kind.FAILED, failedView.kind());
        assertEquals("insufficient_material", failedView.failureReason());
        assertEquals(3, failedView.materialReturned());
        assertEquals(0.25, failedView.qiRefunded());
    }

    @Test
    void craftScreenCompletedUsesSharedFeedbackContract() {
        CraftScreen screen = new CraftScreen();
        List<String> events = new ArrayList<>();
        AtomicInteger sounds = new AtomicInteger();

        // 通过生产 listener 路径驱动：store → screen outcomeListener → shared feedback
        // 测试替换 sound/refresh 观察点不可行（listener 已捕获生产 lambda），
        // 所以断言外部可观察 flashTicks + store lastOutcome + listener 生命周期。
        screen.attachOutcomeListenerForTests();
        assertEquals(0, screen.flashTicksForTests(), "attach 前 flashTicks 应为 0");

        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed(
            "craft.screen.completed", "rough_handle", 1, 42L));

        assertEquals(
            CraftOutcomeFeedback.COMPLETED_FLASH_TICKS,
            screen.flashTicksForTests(),
            "CraftScreen completed 必须设置 flashTicks=6；实际=" + screen.flashTicksForTests()
        );
        assertEquals(
            "craft.screen.completed",
            CraftStore.lastOutcome().orElseThrow().recipeId(),
            "store lastOutcome 必须记录 completed recipe"
        );

        // failed 不得改 flash / 不得“叠加完成反馈”
        int flashAfterCompleted = screen.flashTicksForTests();
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.failed(
            "craft.screen.failed", "player_cancelled", 1, 0.0));
        assertEquals(
            flashAfterCompleted,
            screen.flashTicksForTests(),
            "failed 不得改写已有 flashTicks；实际=" + screen.flashTicksForTests()
        );
        assertEquals(
            CraftStore.CraftOutcomeEvent.Kind.FAILED,
            CraftStore.lastOutcome().orElseThrow().kind(),
            "failed 仍应写入 store"
        );

        // screen removed 后不再消费
        screen.detachOutcomeListenerForTests();
        screen.attachOutcomeListenerForTests(); // re-attach then detach to pin remove semantics
        screen.detachOutcomeListenerForTests();
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed(
            "craft.screen.after_detach", "rough_handle", 1, 99L));
        // detach 后 flash 不应再被新 completed 覆盖为 6（仍可能保留旧值）；
        // 用“再次 attach 前先清 0”更清晰：
        CraftScreen closed = new CraftScreen();
        closed.attachOutcomeListenerForTests();
        closed.detachOutcomeListenerForTests();
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed(
            "craft.screen.closed", "rough_handle", 1, 100L));
        assertEquals(
            0,
            closed.flashTicksForTests(),
            "screen 已关闭（listener 已注销）后 delayed completed 不得再写 flashTicks；实际="
                + closed.flashTicksForTests()
        );
        assertTrue(
            events.isEmpty() && sounds.get() == 0,
            "占位观察点保持空；实际 events=" + events + ", sounds=" + sounds.get()
        );
    }

    @Test
    void workbenchScreenCompletedUsesSharedFeedbackContract() {
        WorkbenchScreen screen = new WorkbenchScreen();
        screen.attachOutcomeListenerForTests();

        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed(
            "wb.screen.completed", "stone_knife", 1, 7L));
        assertEquals(
            CraftOutcomeFeedback.COMPLETED_FLASH_TICKS,
            screen.flashTicksForTests(),
            "WorkbenchScreen completed 必须设置 flashTicks=6；实际=" + screen.flashTicksForTests()
        );

        int flash = screen.flashTicksForTests();
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.failed(
            "wb.screen.failed", "player_cancelled", 1, 0.0));
        assertEquals(
            flash,
            screen.flashTicksForTests(),
            "WorkbenchScreen failed 不得播放完成反馈或改 flashTicks；实际="
                + screen.flashTicksForTests()
        );

        screen.detachOutcomeListenerForTests();
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed(
            "wb.screen.closed", "stone_knife", 1, 8L));
        // 已 detach：flash 保持 failed 后的旧值，不会因新 completed 再次确认/重置流程崩溃
        assertEquals(
            flash,
            screen.flashTicksForTests(),
            "WorkbenchScreen 关闭后 delayed completed 不得再改 flashTicks；实际="
                + screen.flashTicksForTests()
        );
    }


    @Test
    void craftScreenDuplicateAttachPlaysAndRefreshesOnceThenRemovedStopsAllOutcomeEffects() {
        AtomicInteger sounds = new AtomicInteger();
        AtomicInteger refreshes = new AtomicInteger();
        CraftScreen screen = new CraftScreen(sounds::incrementAndGet, refreshes::incrementAndGet);

        screen.attachOutcomeListenerForTests();
        screen.attachOutcomeListenerForTests(); // resize/rebuild 必须幂等
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed(
            "craft.screen.idempotent", "rough_handle", 1, 42L));

        assertEquals(1, sounds.get(),
            "CraftScreen 重复 build/attach 后 completed 仍只能播放一声；实际=" + sounds.get());
        assertEquals(1, refreshes.get(),
            "CraftScreen 重复 build/attach 后 completed 仍只能 refresh 一次；实际=" + refreshes.get());
        assertEquals(CraftOutcomeFeedback.COMPLETED_FLASH_TICKS, screen.flashTicksForTests(),
            "CraftScreen completed 必须设置 flashTicks=6");

        screen.removed();
        int flashAfterRemoval = screen.flashTicksForTests();
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed(
            "craft.screen.after_removed", "rough_handle", 1, 43L));

        assertEquals(1, sounds.get(),
            "CraftScreen removed() 必须注销 outcome listener，之后不得再播音；实际=" + sounds.get());
        assertEquals(1, refreshes.get(),
            "CraftScreen removed() 后不得再 refresh；实际=" + refreshes.get());
        assertEquals(flashAfterRemoval, screen.flashTicksForTests(),
            "CraftScreen removed() 后不得再写 flashTicks；实际=" + screen.flashTicksForTests());
    }

    @Test
    void workbenchScreenDuplicateAttachPlaysAndRefreshesOnceThenRemovedStopsAllOutcomeEffects() {
        AtomicInteger sounds = new AtomicInteger();
        AtomicInteger refreshes = new AtomicInteger();
        WorkbenchScreen screen = new WorkbenchScreen(
            sounds::incrementAndGet,
            refreshes::incrementAndGet
        );

        screen.attachOutcomeListenerForTests();
        screen.attachOutcomeListenerForTests(); // resize/rebuild 必须幂等
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed(
            "workbench.screen.idempotent", "stone_knife", 1, 7L));

        assertEquals(1, sounds.get(),
            "WorkbenchScreen 重复 build/attach 后 completed 仍只能播放一声；实际=" + sounds.get());
        assertEquals(1, refreshes.get(),
            "WorkbenchScreen 重复 build/attach 后 completed 仍只能 refresh 一次；实际=" + refreshes.get());
        assertEquals(CraftOutcomeFeedback.COMPLETED_FLASH_TICKS, screen.flashTicksForTests(),
            "WorkbenchScreen completed 必须设置 flashTicks=6");

        screen.removed();
        int flashAfterRemoval = screen.flashTicksForTests();
        CraftStore.recordOutcome(CraftStore.CraftOutcomeEvent.completed(
            "workbench.screen.after_removed", "stone_knife", 1, 8L));

        assertEquals(1, sounds.get(),
            "WorkbenchScreen removed() 必须注销 outcome listener，之后不得再播音；实际=" + sounds.get());
        assertEquals(1, refreshes.get(),
            "WorkbenchScreen removed() 后不得再 refresh；实际=" + refreshes.get());
        assertEquals(flashAfterRemoval, screen.flashTicksForTests(),
            "WorkbenchScreen removed() 后不得再写 flashTicks；实际=" + screen.flashTicksForTests());
    }

    @Test
    void playerAbsentCompleteSoundIsNoOpWithoutThrowing() {
        // 生产默认音效在 client/player 缺失时必须静默；unit 环境 MinecraftClient.getInstance()
        // 通常为 null，因此直接调用不得抛异常。
        CraftOutcomeFeedback.playDefaultCompleteSound();
    }
}
