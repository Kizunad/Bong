package com.bong.client.hud;

import com.bong.client.combat.store.HalfStepRechallengeStore;

import java.util.ArrayList;
import java.util.List;

/**
 * plan-halfstep-rechallenge-integration-v1 P0：半步化虚重渡触发 HUD。
 *
 * <p>右上角显示「灵机涌现：可重渡虚劫」+ 窗口倒计时。
 * <ul>
 *   <li>主文字颜色 {@code 0xFFE8DFCF}（opacity 0.85 → ARGB alpha = 0xD9）</li>
 *   <li>倒计时正常色 {@code 0xFFE8DFCF}，剩余 &lt;24h 切强调色 {@code 0xFFFF9F5E}</li>
 *   <li>淡入 400ms / 淡出 800ms（窗口过期后 800ms 淡出）</li>
 *   <li>过窗（remainingMs ≤ 0）本地隐藏</li>
 * </ul>
 */
public final class HalfStepRechallengeHudPlanner {

    // ─── 颜色常量 ──────────────────────────────────────────────────
    /** 主文字 ARGB（0xE8DFCF × opacity 0.85 ≈ alpha 0xD9）。 */
    public static final int TEXT_COLOR_MAIN = 0xD9E8DFCF;
    /** 倒计时正常色（同主色，全不透明）。 */
    public static final int TEXT_COLOR_COUNTDOWN = 0xFFE8DFCF;
    /** 剩余 <24h 强调色。 */
    public static final int TEXT_COLOR_URGENT = 0xFFFF9F5E;
    /** 背景色（半透明深色）。 */
    public static final int BG_COLOR = 0xB0101218;

    // ─── 布局常量 ──────────────────────────────────────────────────
    public static final int RIGHT_MARGIN = 8;
    public static final int TOP_MARGIN = 50;
    public static final int BAR_HEIGHT = 20;
    public static final int LINE_HEIGHT = 11;

    // ─── 时间常量 ──────────────────────────────────────────────────
    /** 淡入持续 ms。 */
    public static final long FADE_IN_MS = 400L;
    /** 淡出持续 ms。 */
    public static final long FADE_OUT_MS = 800L;
    /** 24h 强调阈值（ms）。 */
    static final long URGENT_THRESHOLD_MS = 24L * 3600L * 1000L;
    /** 最小显示宽度（px）。 */
    public static final int MIN_BAR_WIDTH = 160;

    private HalfStepRechallengeHudPlanner() {
    }

    public static List<HudRenderCommand> buildCommands(
        int screenWidth,
        int screenHeight,
        long nowMs
    ) {
        List<HudRenderCommand> out = new ArrayList<>();
        HalfStepRechallengeStore.State state = HalfStepRechallengeStore.snapshot();

        if (!state.active()) return out;
        if (screenWidth <= 0 || screenHeight <= 0) return out;

        // 本地过窗判定（含 FADE_OUT_MS 缓冲：fade-out 期间仍显示但透明度递减）
        long remainingMs = state.remainingMs(nowMs);
        long fadeOutElapsed = 0L;
        boolean inFadeOut = false;
        if (remainingMs <= 0L) {
            // 窗口已过——看是否还在淡出动画期
            // remainingMs = 0 时 = 恰好过窗，淡出从此计时。
            // 因 remainingMs 始终 ≥ 0，需借助 triggeredAtMs 估算已过多久。
            long windowDurationMs = (state.windowUntilTick() - state.atTick()) * 50L;
            long elapsedSinceTrigger = Math.max(0L, nowMs - state.triggeredAtMs());
            fadeOutElapsed = Math.max(0L, elapsedSinceTrigger - windowDurationMs);
            if (fadeOutElapsed >= FADE_OUT_MS) {
                // 淡出结束，完全隐藏
                return out;
            }
            inFadeOut = true;
        }

        // alpha 计算
        double alpha;
        long ageMs = Math.max(0L, nowMs - state.triggeredAtMs());
        if (inFadeOut) {
            // 淡出：从 1.0 → 0.0 over FADE_OUT_MS
            alpha = 1.0 - (double) fadeOutElapsed / FADE_OUT_MS;
        } else if (ageMs < FADE_IN_MS) {
            // 淡入：从 0.0 → 1.0 over FADE_IN_MS
            alpha = (double) ageMs / FADE_IN_MS;
        } else {
            alpha = 1.0;
        }
        alpha = Math.max(0.0, Math.min(1.0, alpha));

        // 构建文字
        String line1 = "灵机涌现：可重渡虚劫";
        String line2 = buildCountdownLabel(remainingMs);

        boolean urgent = remainingMs > 0L && remainingMs < URGENT_THRESHOLD_MS;
        int countdownColor = urgent ? TEXT_COLOR_URGENT : TEXT_COLOR_COUNTDOWN;

        // approx 文字宽度
        int approxW = Math.max(MIN_BAR_WIDTH, Math.max(line1.length() * 7, line2.length() * 7));
        int x = screenWidth - approxW - RIGHT_MARGIN;
        x = Math.max(0, x);
        int y = TOP_MARGIN;
        int bgH = BAR_HEIGHT + LINE_HEIGHT + 4;

        // 背景
        HudRenderCommand bg = HudRenderCommand.rect(
            HudRenderLayer.HALFSTEP_RECHALLENGE, x - 4, y - 2, approxW + 8, bgH, BG_COLOR
        );
        // 主文
        HudRenderCommand text1 = HudRenderCommand.text(
            HudRenderLayer.HALFSTEP_RECHALLENGE, line1, x, y + 3, TEXT_COLOR_MAIN
        );
        // 倒计时
        HudRenderCommand text2 = HudRenderCommand.text(
            HudRenderLayer.HALFSTEP_RECHALLENGE, line2, x, y + 3 + LINE_HEIGHT, countdownColor
        );

        // apply alpha
        out.add(HudCommandAlpha.withAlpha(bg, alpha));
        out.add(HudCommandAlpha.withAlpha(text1, alpha));
        out.add(HudCommandAlpha.withAlpha(text2, urgent ? alpha : alpha));

        return out;
    }

    /**
     * 将剩余 ms 转换为倒计时标签。
     * 格式：「剩余 Xd Yh」（≥1d）或「剩余 Yh Zm」（<1d ≥1h）或「剩余 Zm」（<1h）。
     */
    static String buildCountdownLabel(long remainingMs) {
        if (remainingMs <= 0L) {
            return "窗口已关闭";
        }
        long totalSeconds = remainingMs / 1000L;
        long days = totalSeconds / 86400L;
        long hours = (totalSeconds % 86400L) / 3600L;
        long minutes = (totalSeconds % 3600L) / 60L;

        if (days > 0) {
            return "剩余 " + days + "d " + hours + "h";
        } else if (hours > 0) {
            return "剩余 " + hours + "h " + minutes + "m";
        } else {
            return "剩余 " + minutes + "m";
        }
    }
}
