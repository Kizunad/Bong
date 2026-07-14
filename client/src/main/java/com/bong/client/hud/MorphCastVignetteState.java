package com.bong.client.hud;

/**
 * plan-race-system-v1 PR-5b — 易形（{@code morph.yixing}）施法期白色 vignette 的时序状态。
 *
 * <p>由 {@link com.bong.client.visual.particle.MorphVfxPlayer} 在收到
 * {@code bong:morph_yixing} 粒子事件时触发（该事件在 server cast 成功瞬间 emit，
 * 近似"施法期开始"）；{@link MorphHudPlanner} 每帧读取 {@link #alphaAt} 渲染
 * {@code screenTint}。规格（plan §P4 视听规格表）：opacity 峰值 0.15，
 * fade-in 8t（400ms）、fade-out 12t（600ms），总窗口对齐
 * {@code morph_cast.animation.json} endTick=30（1500ms）。
 */
public final class MorphCastVignetteState {
    private static final long FADE_IN_MS = 400L;
    private static final long FADE_OUT_MS = 600L;
    private static final long TOTAL_MS = 1500L;
    private static final double PEAK_OPACITY = 0.15;

    private static volatile long triggeredAtMillis = -1L;

    private MorphCastVignetteState() {}

    public static void trigger(long nowMillis) {
        triggeredAtMillis = nowMillis;
    }

    /** 当前时刻的 vignette 不透明度（{@code 0.0}=不显示）。 */
    public static double alphaAt(long nowMillis) {
        if (triggeredAtMillis < 0) return 0.0;
        long elapsed = nowMillis - triggeredAtMillis;
        if (elapsed < 0 || elapsed > TOTAL_MS) return 0.0;
        if (elapsed < FADE_IN_MS) {
            return PEAK_OPACITY * (elapsed / (double) FADE_IN_MS);
        }
        long fadeOutStart = TOTAL_MS - FADE_OUT_MS;
        if (elapsed >= fadeOutStart) {
            double remaining = TOTAL_MS - elapsed;
            return PEAK_OPACITY * Math.max(0.0, remaining / (double) FADE_OUT_MS);
        }
        return PEAK_OPACITY;
    }

    public static void resetForTests() {
        triggeredAtMillis = -1L;
    }
}
