package com.bong.client.hud;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-race-system-v1 PR-5b — {@link MorphCastVignetteState} 时序单测。
 *
 * <p>规格（plan §P4 视听规格表）：opacity 峰值 0.15，fade-in 8t(400ms)，
 * fade-out 12t(600ms)，总窗口 1500ms。覆盖：① 未触发时恒 0 ② fade-in 期间单调递增
 * ③ hold 期峰值 ④ fade-out 期间单调递减 ⑤ 窗口结束后恒 0 ⑥ 负 elapsed（时钟回拨防御）。
 */
class MorphCastVignetteStateTest {

    @BeforeEach
    void setUp() { MorphCastVignetteState.resetForTests(); }

    @AfterEach
    void tearDown() { MorphCastVignetteState.resetForTests(); }

    @Test
    void neverTriggeredIsAlwaysZero() {
        assertEquals(0.0, MorphCastVignetteState.alphaAt(System.currentTimeMillis()));
    }

    @Test
    void fadeInRampsUpTowardPeak() {
        long t0 = 1_000_000L;
        MorphCastVignetteState.trigger(t0);

        double atStart = MorphCastVignetteState.alphaAt(t0);
        double atMid = MorphCastVignetteState.alphaAt(t0 + 200);
        double atFadeInEnd = MorphCastVignetteState.alphaAt(t0 + 400);

        assertEquals(0.0, atStart, 1e-9, "fade-in 起点应为 0");
        assertTrue(atMid > atStart && atMid < atFadeInEnd, "fade-in 中点应严格介于起止之间（单调递增）");
        assertEquals(0.15, atFadeInEnd, 1e-9, "fade-in 结束应达到峰值 0.15");
    }

    @Test
    void holdPeriodStaysAtPeak() {
        long t0 = 2_000_000L;
        MorphCastVignetteState.trigger(t0);
        // fade-in(400ms) 结束到 fade-out(600ms) 开始之间是 hold 期（1500-600=900ms 起点）。
        assertEquals(0.15, MorphCastVignetteState.alphaAt(t0 + 600), 1e-9);
        assertEquals(0.15, MorphCastVignetteState.alphaAt(t0 + 899), 1e-9);
    }

    @Test
    void fadeOutRampsDownToZero() {
        long t0 = 3_000_000L;
        MorphCastVignetteState.trigger(t0);
        long fadeOutStart = t0 + 900; // 1500-600
        double atStart = MorphCastVignetteState.alphaAt(fadeOutStart);
        double atMid = MorphCastVignetteState.alphaAt(fadeOutStart + 300);
        double atEnd = MorphCastVignetteState.alphaAt(t0 + 1500);

        assertEquals(0.15, atStart, 1e-9, "fade-out 起点应仍在峰值");
        assertTrue(atMid < atStart && atMid > atEnd, "fade-out 中点应严格介于起止之间（单调递减）");
        assertEquals(0.0, atEnd, 1e-9, "窗口结束应回到 0");
    }

    @Test
    void afterWindowEndsAlphaIsZero() {
        long t0 = 4_000_000L;
        MorphCastVignetteState.trigger(t0);
        assertEquals(0.0, MorphCastVignetteState.alphaAt(t0 + 1501));
        assertEquals(0.0, MorphCastVignetteState.alphaAt(t0 + 100_000));
    }

    @Test
    void negativeElapsedFromClockSkewIsSafelyZero() {
        long t0 = 5_000_000L;
        MorphCastVignetteState.trigger(t0);
        // now 早于触发时刻（时钟回拨防御场景），不应返回负值或抛异常。
        assertEquals(0.0, MorphCastVignetteState.alphaAt(t0 - 100));
    }

    @Test
    void disconnectClearDropsOldCastWindowAndAllowsFreshCast() {
        long oldCast = 7_000_000L;
        MorphCastVignetteState.trigger(oldCast);
        assertEquals(0.15, MorphCastVignetteState.alphaAt(oldCast + 400L), 1e-9,
            "old cast must be visible before disconnect cleanup");

        MorphCastVignetteState.clearOnDisconnect();

        assertEquals(0.0, MorphCastVignetteState.alphaAt(oldCast + 400L), 1e-9,
            "old cast vignette must not bleed into a new connection");
        long freshCast = oldCast + 10_000L;
        MorphCastVignetteState.trigger(freshCast);
        assertEquals(0.15, MorphCastVignetteState.alphaAt(freshCast + 400L), 1e-9,
            "fresh cast must keep the normal vignette timing after teardown");
    }

    @Test
    void retriggerResetsWindow() {
        long t0 = 6_000_000L;
        MorphCastVignetteState.trigger(t0);
        assertEquals(0.0, MorphCastVignetteState.alphaAt(t0 + 1600), "第一次窗口应已结束");

        long t1 = t0 + 5000;
        MorphCastVignetteState.trigger(t1);
        assertEquals(0.0, MorphCastVignetteState.alphaAt(t1), "重新触发后窗口从 0 起算");
        assertEquals(0.15, MorphCastVignetteState.alphaAt(t1 + 400), 1e-9);
    }
}
