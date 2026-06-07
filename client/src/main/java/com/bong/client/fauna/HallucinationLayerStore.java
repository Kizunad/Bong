package com.bong.client.fauna;

/**
 * plan-fauna-stitched-beast-v1 P3 — 客户端幻觉层全局状态存储。
 *
 * <p>由 {@link HallucinationLayerHandler} 写入，由 {@link HallucinationHudOverlay} 读取。
 *
 * <p><b>守恒红线</b>：此存储仅管理 <em>显示层</em> 状态（视野旋转幅度/bar偏移/fade进度），
 * <strong>绝不</strong>持有或修改玩家实际 HP / qi_current 值。
 *
 * <p>线程安全：{@code volatile} 字段保证渲染线程读取的可见性；写入仅发生在 Minecraft 主线程
 * （{@code client.execute()} 内）。
 */
public final class HallucinationLayerStore {

    // ── 激活状态 ─────────────────────────────────────────────────────────────

    /**
     * 幻觉当前是否激活（已收到 server S2C payload 且 duration > 0）。
     * false = 不激活，overlay 应淡出直到消失。
     */
    private static volatile boolean active = false;

    /**
     * 幻觉剩余 tick 数（从 duration_ticks 开始递减；到达 0 时 active 置 false）。
     * 每帧由 {@link HallucinationHudOverlay} 更新（不依赖 server 推取消包）。
     */
    private static volatile int remainingTicks = 0;

    /**
     * 幻觉总持续 tick 数（由 payload 写入；用于计算 fade-out 开始时机）。
     * P3 固定 200（10s @ 20TPS）。
     */
    private static volatile int durationTicks = 0;

    // ── Fade 进度（0.0 = 透明，1.0 = 完整目标强度）─────────────────────────

    /**
     * 当前 fade 进度（0.0 - 1.0）。
     * <ul>
     *   <li>fade-in：0→1（10tick 线性）</li>
     *   <li>稳定段：1.0</li>
     *   <li>fade-out：1→0（20tick 线性）</li>
     * </ul>
     */
    private static volatile float fadeProgress = 0.0f;

    // ── Bar 偏移（每 10 tick 重随机）──────────────────────────────────────────

    /**
     * HP bar 显示偏移量（-0.2 ~ +0.2，纯显示层，不改实际值）。
     * 每 10 tick 在 HallucinationHudOverlay 重新随机。
     */
    private static volatile float hpBarDisplayOffset = 0.0f;

    /**
     * qi bar 显示偏移量（-0.2 ~ +0.2，纯显示层，不改实际值）。
     * 每 10 tick 在 HallucinationHudOverlay 重新随机。
     */
    private static volatile float qiBarDisplayOffset = 0.0f;

    // ── 当前 sin wave 相位（视野旋转）─────────────────────────────────────────

    /**
     * 当前 sin wave 相位（0 ~ 2π，每帧累进）。
     * 用于视野偏航旋转：yaw = sin(phase) × MAX_YAW_DEGREES。
     */
    private static volatile float sinPhase = 0.0f;

    private HallucinationLayerStore() {}

    // ── 读取 API ─────────────────────────────────────────────────────────────

    /** 幻觉是否当前激活。 */
    public static boolean isActive() {
        return active;
    }

    /** 剩余 tick 数（到 0 时自动停用）。 */
    public static int getRemainingTicks() {
        return remainingTicks;
    }

    /** 总持续 tick 数（用于 fade-out 时机计算）。 */
    public static int getDurationTicks() {
        return durationTicks;
    }

    /** 当前 fade 进度（0.0 = 不可见，1.0 = 完整显示）。 */
    public static float getFadeProgress() {
        return fadeProgress;
    }

    /** HP bar 显示偏移量（-0.2 ~ +0.2）。 */
    public static float getHpBarDisplayOffset() {
        return hpBarDisplayOffset;
    }

    /** Qi bar 显示偏移量（-0.2 ~ +0.2）。 */
    public static float getQiBarDisplayOffset() {
        return qiBarDisplayOffset;
    }

    /** Sin wave 相位（视野旋转使用）。 */
    public static float getSinPhase() {
        return sinPhase;
    }

    // ── 写入 API（仅供 Handler / Overlay 调用）──────────────────────────────

    /**
     * 激活幻觉（由 {@link HallucinationLayerHandler} 在 Minecraft 主线程调用）。
     *
     * @param durationTicksValue 幻觉持续 tick 数（P3 固定 200）
     */
    public static void activate(int durationTicksValue) {
        durationTicks = durationTicksValue;
        remainingTicks = durationTicksValue;
        active = true;
        // fade-in 从头开始（不延续上次 fade 进度）
        fadeProgress = 0.0f;
        sinPhase = 0.0f;
    }

    /**
     * 立即取消幻觉（断线 / 收到 cancel payload 时调用）。
     */
    public static void cancel() {
        active = false;
        remainingTicks = 0;
        fadeProgress = 0.0f;
    }

    /**
     * 每帧更新（由 {@link HallucinationHudOverlay} 在渲染循环内调用）。
     *
     * <p>更新逻辑：
     * <ol>
     *   <li>若 active：fade-in（remainingTicks > 0），fade-out（remainingTicks <= 0）</li>
     *   <li>若 inactive：fade-out</li>
     *   <li>sin wave 相位累进</li>
     * </ol>
     *
     * @param sinPhaseIncrement  每帧 sin phase 增量（弧度），由 Overlay 按帧率计算传入
     * @param fadeInStep         fade-in 每帧增量（范围 0~1）
     * @param fadeOutStep        fade-out 每帧减量（范围 0~1）
     */
    public static void tickFade(float sinPhaseIncrement, float fadeInStep, float fadeOutStep) {
        if (active && remainingTicks > 0) {
            // fade-in 阶段
            fadeProgress = Math.min(1.0f, fadeProgress + fadeInStep);
        } else {
            // fade-out 阶段（inactive 或 remainingTicks 耗尽）
            fadeProgress = Math.max(0.0f, fadeProgress - fadeOutStep);
            if (fadeProgress <= 0.0f) {
                active = false;
            }
        }
        sinPhase += sinPhaseIncrement;
        if (sinPhase > (float) (2 * Math.PI)) {
            sinPhase -= (float) (2 * Math.PI);
        }
    }

    /**
     * 递减剩余 tick 数（由 Overlay 每 tick 调用；到 0 时 active 置 false 开始 fade-out）。
     */
    public static void decrementTick() {
        if (remainingTicks > 0) {
            remainingTicks--;
        }
        if (remainingTicks <= 0 && active) {
            // 进入 fade-out 阶段（不立即设 active=false，等 fade 完成）
        }
    }

    /**
     * 更新 bar 显示偏移量（每 10 tick 随机一次，由 Overlay 调用）。
     *
     * @param hp  HP bar 偏移（-0.2 ~ +0.2）
     * @param qi  qi bar 偏移（-0.2 ~ +0.2）
     */
    public static void updateBarOffsets(float hp, float qi) {
        hpBarDisplayOffset = hp;
        qiBarDisplayOffset = qi;
    }

    /**
     * 断线清理（由 BongNetworkHandler onDisconnect 调用）。
     */
    public static void clearOnDisconnect() {
        active = false;
        remainingTicks = 0;
        durationTicks = 0;
        fadeProgress = 0.0f;
        hpBarDisplayOffset = 0.0f;
        qiBarDisplayOffset = 0.0f;
        sinPhase = 0.0f;
    }

    // ── 仅供测试的 accessor ──────────────────────────────────────────────────

    /** 仅供测试：完全重置所有状态。 */
    static void resetForTest() {
        clearOnDisconnect();
    }

    /** 仅供测试：直接注入 fadeProgress。 */
    static void setFadeProgressForTest(float value) {
        fadeProgress = Math.max(0.0f, Math.min(1.0f, value));
    }

    /** 仅供测试：直接注入 remainingTicks。 */
    static void setRemainingTicksForTest(int ticks) {
        remainingTicks = ticks;
    }

    /** 仅供测试：直接注入 active。 */
    static void setActiveForTest(boolean value) {
        active = value;
    }
}
