package com.bong.client.era;

import java.util.concurrent.atomic.AtomicReference;

/**
 * plan-era-state-v1 P3 — 时代天象客户端状态存储（thread-safe singleton）。
 *
 * <p>保存当前生效的 {@link EraAmbiancePayload} 及渐变进度。
 * 渲染层通过 {@link #current()} 读取，{@link EraAmbianceHandler} 通过 {@link #accept(EraAmbiancePayload)}
 * 写入。
 *
 * <h3>线性插值规则</h3>
 * <p>收到新 payload 时将 {@code elapsedTicks} 重置为 0，{@code targetPayload} 更新。
 * 每 game tick 调用 {@link #tick()} 推进计数器（应在主 tick loop 中调用）。
 * {@link #interpolateSkyTintRgb(int)} 和 {@link #interpolateFogDensityDelta()} 按
 * {@code elapsedTicks / transitionTicks} 线性插值，从 baseline（zone profile 值）过渡到目标值。
 *
 * <p>注意：此类不做 ZoneAtmosphere 的直接修改，只提供数据供 planner 读取。
 */
public final class EraAmbianceState {

    private static final AtomicReference<EraAmbianceState> INSTANCE =
        new AtomicReference<>(new EraAmbianceState());

    private volatile EraAmbiancePayload previous;
    private volatile EraAmbiancePayload target;
    private volatile int elapsedTicks;

    private EraAmbianceState() {
    }

    // ── 写入（由 handler 调用） ──────────────────────────────────────────────────

    /**
     * 接受新的 era 天象 payload，开始渐变过渡。
     * 线程安全：使用 volatile 保证可见性（handler 在 Netty IO 线程，tick 在游戏主线程）。
     */
    public static void accept(EraAmbiancePayload payload) {
        EraAmbianceState state = INSTANCE.get();
        // 当前 target 成为 previous（用于插值起点），重置计数器
        state.previous = state.target;
        state.target = payload;
        state.elapsedTicks = 0;
    }

    // ── 读取（由渲染层调用） ─────────────────────────────────────────────────────

    /**
     * 返回当前 target payload，若无返回 null（无时代宣告时）。
     */
    public static EraAmbiancePayload current() {
        return INSTANCE.get().target;
    }

    /**
     * 按渐变进度插值 sky_tint_rgb。
     *
     * <p>若 target 为 null 或为 reset，返回 {@code baseRgb}（zone profile 原值）。
     * 若 target.skyTintRgb == 0 (baseline)，返回 {@code baseRgb}。
     *
     * @param baseRgb zone profile 的 skyTintRgb（插值起点）
     * @return 插值后的 skyTintRgb
     */
    public static int interpolateSkyTintRgb(int baseRgb) {
        EraAmbianceState state = INSTANCE.get();
        EraAmbiancePayload tgt = state.target;
        if (tgt == null || tgt.isReset() || !tgt.hasSkyTint()) {
            return baseRgb;
        }
        double t = state.transitionProgress(tgt.transitionTicks());
        return blendRgb(baseRgb, tgt.skyTintRgb(), t);
    }

    /**
     * 按渐变进度插值 fog_density_delta。
     *
     * <p>若 target 为 null 或为 reset，返回 0.0。
     *
     * @return 插值后的 fog density 增量（叠加到 zone profile 的 fog_density）
     */
    public static double interpolateFogDensityDelta() {
        EraAmbianceState state = INSTANCE.get();
        EraAmbiancePayload tgt = state.target;
        if (tgt == null || tgt.isReset()) {
            return 0.0;
        }
        double t = state.transitionProgress(tgt.transitionTicks());
        return tgt.fogDensityDelta() * t;
    }

    /**
     * 游戏主 tick 推进渐变计数器。
     * 应在每 game tick（约 50ms）调用一次。
     */
    public static void tick() {
        EraAmbianceState state = INSTANCE.get();
        EraAmbiancePayload tgt = state.target;
        if (tgt != null && state.elapsedTicks < tgt.transitionTicks()) {
            state.elapsedTicks++;
        }
    }

    /**
     * 重置状态（用于断开连接或测试清理）。
     */
    public static void reset() {
        INSTANCE.set(new EraAmbianceState());
    }

    // ── 辅助 ─────────────────────────────────────────────────────────────────────

    private double transitionProgress(int transitionTicks) {
        if (transitionTicks <= 0) {
            return 1.0;
        }
        return Math.min(1.0, (double) elapsedTicks / transitionTicks);
    }

    /**
     * 线性插值两个 RGB int。{@code t=0} 返回 {@code from}，{@code t=1} 返回 {@code to}。
     */
    static int blendRgb(int from, int to, double t) {
        double clampT = Math.max(0.0, Math.min(1.0, t));
        int r = (int) Math.round(((from >> 16) & 0xFF) * (1.0 - clampT) + ((to >> 16) & 0xFF) * clampT);
        int g = (int) Math.round(((from >> 8) & 0xFF) * (1.0 - clampT) + ((to >> 8) & 0xFF) * clampT);
        int b = (int) Math.round((from & 0xFF) * (1.0 - clampT) + (to & 0xFF) * clampT);
        return (clamp255(r) << 16) | (clamp255(g) << 8) | clamp255(b);
    }

    private static int clamp255(int val) {
        return Math.max(0, Math.min(255, val));
    }
}
