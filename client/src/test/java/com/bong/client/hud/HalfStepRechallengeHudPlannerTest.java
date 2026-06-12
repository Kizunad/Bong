package com.bong.client.hud;

import com.bong.client.combat.store.HalfStepRechallengeStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-halfstep-rechallenge-integration-v1 P0：HalfStepRechallengeHudPlanner 单测。
 * 覆盖：显示时机 / 倒计时计算 / 颜色切换（<24h 边界）/ 淡入触发 / 三终止淡出 / 非 HalfStep 不显示。
 */
class HalfStepRechallengeHudPlannerTest {

    // ─── Constants for convenience ─────────────────────────────────
    /** atTick=0, windowUntilTick=7d*20tps = 12096000 ticks  (7 days exactly). */
    private static final long SEVEN_DAY_TICKS = 7L * 24L * 3600L * 20L; // 12_096_000
    private static final long ONE_DAY_TICKS = 24L * 3600L * 20L; // 1_728_000

    @AfterEach
    void tearDown() {
        HalfStepRechallengeStore.resetForTests();
    }

    // ─── 1. 非 HalfStep 状态不显示任何命令 ─────────────────────────────
    @Test
    void hiddenWhenStoreIsEmpty() {
        // default state = NONE (active=false)
        List<HudRenderCommand> cmds = HalfStepRechallengeHudPlanner.buildCommands(800, 600, 1_000L);
        assertTrue(cmds.isEmpty(),
            "Expected no commands when store is NONE (active=false), got: " + cmds.size());
    }

    @Test
    void hiddenWhenScreenDimensionsAreZero() {
        seedActiveState(SEVEN_DAY_TICKS, 0L, 0L);
        List<HudRenderCommand> cmds = HalfStepRechallengeHudPlanner.buildCommands(0, 0, 0L);
        assertTrue(cmds.isEmpty(),
            "Expected no commands when screenWidth=0 screenHeight=0");
    }

    // ─── 2. 正常显示（全不透明，主文字存在） ─────────────────────────────
    @Test
    void showsMainTextWhenActive() {
        long triggeredAt = 1000L;
        seedActiveState(SEVEN_DAY_TICKS, 0L, triggeredAt);
        // nowMs = triggeredAt + FADE_IN_MS + 10 → full alpha
        long nowMs = triggeredAt + HalfStepRechallengeHudPlanner.FADE_IN_MS + 10L;
        List<HudRenderCommand> cmds = HalfStepRechallengeHudPlanner.buildCommands(800, 600, nowMs);
        assertFalse(cmds.isEmpty(),
            "Expected commands to be present when active and past fade-in");
        boolean hasMainText = cmds.stream().anyMatch(c -> c.isText()
            && c.text().contains("灵机涌现：可重渡虚劫"));
        assertTrue(hasMainText,
            "Expected main text '灵机涌现：可重渡虚劫' to appear; got texts: "
                + cmds.stream().filter(HudRenderCommand::isText).map(HudRenderCommand::text).toList());
    }

    // ─── 3. 倒计时标签格式 ────────────────────────────────────────────
    @Test
    void countdownLabelMultipleDays() {
        // 3d 5h remaining → 3*86400 + 5*3600 = 277200s = 277200000ms
        long ms = (3L * 86400L + 5L * 3600L) * 1000L;
        assertEquals("剩余 3d 5h",
            HalfStepRechallengeHudPlanner.buildCountdownLabel(ms),
            "Expected 'days + hours' format for >=1 day remaining");
    }

    @Test
    void countdownLabelHoursMinutes() {
        // 5h 20m = 5*3600 + 20*60 = 19200s
        long ms = (5L * 3600L + 20L * 60L) * 1000L;
        assertEquals("剩余 5h 20m",
            HalfStepRechallengeHudPlanner.buildCountdownLabel(ms),
            "Expected 'hours + minutes' format for <1d >=1h remaining");
    }

    @Test
    void countdownLabelMinutesOnly() {
        // 45m = 45*60 = 2700s
        long ms = 45L * 60L * 1000L;
        assertEquals("剩余 45m",
            HalfStepRechallengeHudPlanner.buildCountdownLabel(ms),
            "Expected 'minutes only' format for <1h remaining");
    }

    @Test
    void countdownLabelExpired() {
        assertEquals("窗口已关闭",
            HalfStepRechallengeHudPlanner.buildCountdownLabel(0L),
            "Expected '窗口已关闭' when remainingMs=0");
        assertEquals("窗口已关闭",
            HalfStepRechallengeHudPlanner.buildCountdownLabel(-1L),
            "Expected '窗口已关闭' when remainingMs<0");
    }

    // ─── 4. 颜色切换边界：<24h 切强调色 ─────────────────────────────────
    @Test
    void normalColorWhenMoreThan24hRemaining() {
        // windowUntilTick = atTick + 25*ONE_DAY/50ms = 25*3600*20 ticks
        long triggeredAt = 5000L;
        long windowTicks = 25L * ONE_DAY_TICKS; // 25 days remaining
        seedActiveState(windowTicks, 0L, triggeredAt);
        long nowMs = triggeredAt + HalfStepRechallengeHudPlanner.FADE_IN_MS + 1L;
        List<HudRenderCommand> cmds = HalfStepRechallengeHudPlanner.buildCommands(800, 600, nowMs);
        // Countdown text should use normal color (TEXT_COLOR_COUNTDOWN = 0xFFE8DFCF = -1712177)
        boolean hasNormalCountdown = cmds.stream().anyMatch(c ->
            c.isText()
                && c.text().startsWith("剩余")
                && (c.color() & 0x00FFFFFF) == (HalfStepRechallengeHudPlanner.TEXT_COLOR_COUNTDOWN & 0x00FFFFFF)
        );
        assertTrue(hasNormalCountdown,
            "Expected countdown text with normal color 0xFFE8DFCF when >24h remaining; colors: "
                + cmds.stream().filter(c -> c.isText() && c.text().startsWith("剩余"))
                    .map(c -> Integer.toHexString(c.color())).toList());
    }

    @Test
    void urgentColorWhenLessThan24hRemaining() {
        // window = 20h remaining = 20*3600*20 ticks
        long triggeredAt = 5000L;
        long windowTicks = 20L * 3600L * 20L; // 20h of ticks
        seedActiveState(windowTicks, 0L, triggeredAt);
        // nowMs very close to triggeredAt so window hasn't elapsed yet
        long nowMs = triggeredAt + HalfStepRechallengeHudPlanner.FADE_IN_MS + 1L;
        List<HudRenderCommand> cmds = HalfStepRechallengeHudPlanner.buildCommands(800, 600, nowMs);
        boolean hasUrgentCountdown = cmds.stream().anyMatch(c ->
            c.isText()
                && c.text().startsWith("剩余")
                && (c.color() & 0x00FFFFFF) == (HalfStepRechallengeHudPlanner.TEXT_COLOR_URGENT & 0x00FFFFFF)
        );
        assertTrue(hasUrgentCountdown,
            "Expected countdown text with urgent color 0xFFFF9F5E when <24h remaining; colors: "
                + cmds.stream().filter(c -> c.isText() && c.text().startsWith("剩余"))
                    .map(c -> Integer.toHexString(c.color())).toList());
    }

    // ─── 5. 淡入：触发后第一帧 alpha < 1.0 ───────────────────────────────
    @Test
    void fadeInProducesPartialAlphaAtStart() {
        long triggeredAt = 10_000L;
        seedActiveState(SEVEN_DAY_TICKS, 0L, triggeredAt);
        // nowMs = triggeredAt + 200ms → halfway through 400ms fade-in → alpha ≈ 0.5
        long nowMs = triggeredAt + 200L;
        List<HudRenderCommand> cmds = HalfStepRechallengeHudPlanner.buildCommands(800, 600, nowMs);
        assertFalse(cmds.isEmpty(), "Expected partial-alpha commands during fade-in");
        // All text commands should have reduced alpha (< 0xFF_000000 component)
        boolean hasPartialAlphaText = cmds.stream().anyMatch(c ->
            c.isText() && ((c.color() >>> 24) & 0xFF) < 200
        );
        assertTrue(hasPartialAlphaText,
            "Expected at least one text command with alpha < 200 (partial fade-in at 200ms/400ms); "
                + "text command alphas: "
                + cmds.stream().filter(HudRenderCommand::isText)
                    .map(c -> String.valueOf((c.color() >>> 24) & 0xFF)).toList());
    }

    // ─── 6. 三终止条件：过窗后 800ms 淡出，完全隐藏 ──────────────────────
    @Test
    void fadeOutAfterWindowExpiry() {
        // window = 100ms (tiny), triggeredAt = 0
        // windowUntilTick = 2 ticks (100ms), atTick = 0
        long triggeredAt = 0L;
        long windowTicks = 2L; // 2 ticks × 50ms = 100ms
        seedActiveState(windowTicks, 0L, triggeredAt);

        // At nowMs = 800ms, window expired 700ms ago → fadeOutElapsed = 700ms < 800ms → still visible
        long midFadeOut = triggeredAt + 100L + 700L;
        List<HudRenderCommand> cmds800 = HalfStepRechallengeHudPlanner.buildCommands(800, 600, midFadeOut);
        assertFalse(cmds800.isEmpty(),
            "Expected commands to still be present during fade-out (700ms elapsed of 800ms)");

        // At nowMs = triggeredAt + 100ms window + 800ms fadeOut = 900ms → fade-out complete → empty
        long afterFadeOut = triggeredAt + 100L + HalfStepRechallengeHudPlanner.FADE_OUT_MS;
        List<HudRenderCommand> cmdsAfter = HalfStepRechallengeHudPlanner.buildCommands(800, 600, afterFadeOut);
        assertTrue(cmdsAfter.isEmpty(),
            "Expected commands to be empty after full fade-out (window + FADE_OUT_MS elapsed)");
    }

    @Test
    void hiddenImmediatelyAfterExplicitClear() {
        seedActiveState(SEVEN_DAY_TICKS, 0L, 0L);
        // Confirm it's visible first
        List<HudRenderCommand> before = HalfStepRechallengeHudPlanner.buildCommands(
            800, 600, HalfStepRechallengeHudPlanner.FADE_IN_MS + 10L
        );
        assertFalse(before.isEmpty(), "Should be visible before clear");

        // Server sends active=false → clear()
        HalfStepRechallengeStore.clear();

        List<HudRenderCommand> after = HalfStepRechallengeHudPlanner.buildCommands(
            800, 600, HalfStepRechallengeHudPlanner.FADE_IN_MS + 10L
        );
        assertTrue(after.isEmpty(),
            "Expected no commands after store.clear() (explicit HIDE from server)");
    }

    // ─── 7. buildCountdownLabel shows in-game countdown from store state ──
    @Test
    void countdownInCommandReflectsStoreWindowField() {
        // 5-day window so even after FADE_IN_MS elapsed the countdown still shows "4d"+ or "5d".
        // exact: windowTicks * 50ms - FADE_IN_MS - 1ms = 5 * 86400000 - 401 = 431999599ms → 4d 23h
        long triggeredAt = 0L;
        long windowTicks = 5L * ONE_DAY_TICKS;
        seedActiveState(windowTicks, 0L, triggeredAt);
        long nowMs = triggeredAt + HalfStepRechallengeHudPlanner.FADE_IN_MS + 1L;
        List<HudRenderCommand> cmds = HalfStepRechallengeHudPlanner.buildCommands(800, 600, nowMs);
        // Remaining ≈ 5 days - 401ms → "4d 23h". Just check "d" appears (multi-day format).
        boolean hasMultiDayCountdown = cmds.stream().anyMatch(c -> c.isText()
            && c.text().startsWith("剩余")
            && c.text().contains("d"));
        assertTrue(hasMultiDayCountdown,
            "Expected multi-day countdown '剩余 Xd Yh' in commands for 5-day window; texts: "
                + cmds.stream().filter(HudRenderCommand::isText).map(HudRenderCommand::text).toList());
    }

    // ─── 8. 全命令使用正确的 layer ──────────────────────────────────────
    @Test
    void allCommandsUseHalfStepRechallengeLayer() {
        seedActiveState(SEVEN_DAY_TICKS, 0L, 0L);
        long nowMs = HalfStepRechallengeHudPlanner.FADE_IN_MS + 10L;
        List<HudRenderCommand> cmds = HalfStepRechallengeHudPlanner.buildCommands(800, 600, nowMs);
        assertFalse(cmds.isEmpty());
        boolean allCorrectLayer = cmds.stream().allMatch(c ->
            c.layer() == HudRenderLayer.HALFSTEP_RECHALLENGE
        );
        assertTrue(allCorrectLayer,
            "Expected all commands to use HALFSTEP_RECHALLENGE layer; layers: "
                + cmds.stream().map(c -> c.layer().name()).toList());
    }

    // ─── helpers ────────────────────────────────────────────────────

    /**
     * Seeds the store with an active state.
     *
     * @param windowUntilTick  absolute tick at which the window closes (= atTick + window_ticks)
     * @param atTick           tick at trigger time (usually 0 in tests)
     * @param triggeredAtMs    wall-clock ms when the trigger arrived (used for fade timing)
     */
    private static void seedActiveState(long windowUntilTick, long atTick, long triggeredAtMs) {
        HalfStepRechallengeStore.replace(
            new HalfStepRechallengeStore.State(true, "offline:Azure", windowUntilTick, atTick, triggeredAtMs)
        );
    }
}
