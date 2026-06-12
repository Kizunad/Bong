package com.bong.client.agentui;

/**
 * plan-agent-ui-data-v1 P3 — 天道动态面板 VFX 状态。
 *
 * <p>记录面板打开时间和面板类型，供 {@link AgentUiVfxPlanner} 生成逐帧 HUD 覆层命令。
 *
 * <p>VFX 规格（plan §5）：
 * <ul>
 *   <li>fade-in：面板整体 opacity 0→1，duration 10t（500ms），easing linear</li>
 *   <li>天道启示特有 vignette：颜色 {@code #1A0D40}（暗蓝），opacity 0.4，200t（10s）</li>
 *   <li>天道启示特有 shake：amplitude 0.5px，frequency 0.1Hz，duration 40t（2s）</li>
 * </ul>
 *
 * @param openedAtMs         面板打开时的系统时间戳（ms），用于计算动画进度
 * @param isTiandaoRevelation 是否为天道启示面板（realm_gate=5），启用特有 vignette+shake
 */
public record AgentUiVfxState(
    long openedAtMs,
    boolean isTiandaoRevelation
) {
    /** 面板整体 fade-in 持续时间（ms）= 10t × 50ms/t */
    public static final long FADE_IN_DURATION_MS = 500L;

    /** 天道启示 vignette 颜色（#1A0D40，ARGB: alpha 由 opacity 计算） */
    public static final int TIANDAO_VIGNETTE_RGB = 0x1A0D40;

    /** 天道启示 vignette opacity = 0.4 → alpha = round(0.4 × 255) = 102 → 0x66 */
    public static final int TIANDAO_VIGNETTE_ALPHA = 0x66;

    /** 天道启示 vignette ARGB = 0x661A0D40 */
    public static final int TIANDAO_VIGNETTE_ARGB = (TIANDAO_VIGNETTE_ALPHA << 24) | TIANDAO_VIGNETTE_RGB;

    /** 天道启示 vignette 持续时间（ms）= 200t × 50ms/t */
    public static final long TIANDAO_VIGNETTE_DURATION_MS = 10_000L;

    /** 天道启示 shake 持续时间（ms）= 40t × 50ms/t */
    public static final long TIANDAO_SHAKE_DURATION_MS = 2_000L;

    /**
     * 在指定时刻计算 fade-in 进度（0.0→1.0 linear，到达 FADE_IN_DURATION_MS 后恒为 1.0）。
     *
     * @param nowMs 当前时间戳（ms）
     * @return 0.0（全透明）到 1.0（完全不透明）
     */
    public double fadeInProgress(long nowMs) {
        long elapsed = nowMs - openedAtMs;
        if (FADE_IN_DURATION_MS <= 0L) return 1.0;
        return Math.max(0.0, Math.min(1.0, elapsed / (double) FADE_IN_DURATION_MS));
    }

    /**
     * 天道启示 vignette 当前是否活跃（duration 内）。
     *
     * @param nowMs 当前时间戳（ms）
     */
    public boolean tiandaoVignetteActive(long nowMs) {
        if (!isTiandaoRevelation) return false;
        return nowMs - openedAtMs < TIANDAO_VIGNETTE_DURATION_MS;
    }

    /**
     * 天道启示 shake 当前是否活跃（前 TIANDAO_SHAKE_DURATION_MS 内）。
     *
     * @param nowMs 当前时间戳（ms）
     */
    public boolean tiandaoShakeActive(long nowMs) {
        if (!isTiandaoRevelation) return false;
        return nowMs - openedAtMs < TIANDAO_SHAKE_DURATION_MS;
    }
}
