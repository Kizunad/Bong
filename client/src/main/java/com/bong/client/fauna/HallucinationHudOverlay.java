package com.bong.client.fauna;

import net.minecraft.client.gui.DrawContext;

/**
 * plan-fauna-stitched-beast-v1 P3 — 兽核吸收幻觉 HUD overlay。
 *
 * <p>在玩家使用 {@code bian_yi_hexin}（变异核心）后触发，持续 200 tick（10s @ 20TPS）：
 * <ul>
 *   <li><b>绿色边缘像差</b>：四边缘 fillGradient（颜色 #80F040，半透明），随 fade 曲线渐变</li>
 *   <li><b>HP / qi bar 显示偏移</b>：±20% 随机偏移，每 10 tick 重随机，<strong>绝不改实际值</strong></li>
 *   <li><b>视野旋转</b>：sin wave ±3° yaw 偏移（period 约 40 tick）——视觉观感需用户 WSLg 手测验收</li>
 *   <li><b>音效</b>：ambient.cave（vanilla）pitch 0.5→1.2 渐变，由 {@link HallucinationTickController} 每 tick 触发</li>
 *   <li><b>Fade</b>：进入 10 tick（0→1），退出 20 tick（1→0）</li>
 * </ul>
 *
 * <p><b>Tick 架构（M2 blocker fix）</b>：本类 {@link #render} 仅<em>读取</em> {@link HallucinationLayerStore}
 * 状态做渲染，不修改任何计数器。所有递减/进度更新（{@code decrementTick} / {@code tickFade} /
 * bar 偏移重随机 / 音效）均由 {@link HallucinationTickController}（挂在
 * {@code ClientTickEvents.END_CLIENT_TICK}，20Hz）驱动，确保 200 tick = 10s 与 server 对齐、帧率无关。
 *
 * <p><b>守恒红线</b>：所有 bar 偏移仅用于 HUD 渲染时的 <em>显示值</em>，
 * 不经过任何写回玩家 HP / qi 的路径。实际 HUD 数值来源不变。
 *
 * <p><b>视觉待验收</b>：视野旋转 / 绿边像差 / bar 偏移需用户 WSLg 手测验收。
 *
 * <p>渲染接入点：{@link com.bong.client.BongHud} 里 {@code visibility == FULL} 分支。
 */
public final class HallucinationHudOverlay {

    // ──────────────────────────────────────────────────────────────────────────
    // 颜色常量（ARGB 格式）
    // ──────────────────────────────────────────────────────────────────────────

    /**
     * 绿色边缘像差 ARGB 颜色（#80F040，半透明绿色，象征异变兽核共鸣）。
     * alpha=0x60（约 37.5%），RGB=(0x80,0xF0,0x40)。
     */
    static final int HALLUCINATION_EDGE_COLOR_MAX = 0x6080F040;

    /** 透明色（用于 fillGradient 渐变到透明）。 */
    static final int TRANSPARENT = 0x00000000;

    // ──────────────────────────────────────────────────────────────────────────
    // Fade 曲线参数（tick-based，供 HallucinationTickController 使用）
    // ──────────────────────────────────────────────────────────────────────────

    /**
     * fade-in 每 tick 增量（10 tick 完成，1.0 / 10 = 0.1）。
     * 由 {@link HallucinationTickController} 每游戏 tick 调用，与帧率无关。
     */
    static final float FADE_IN_STEP_PER_TICK = 1.0f / 10.0f;

    /**
     * fade-out 每 tick 减量（20 tick 完成，1.0 / 20 = 0.05）。
     * 由 {@link HallucinationTickController} 每游戏 tick 调用，与帧率无关。
     */
    static final float FADE_OUT_STEP_PER_TICK = 1.0f / 20.0f;

    /**
     * @deprecated 仅供旧测试引用。请使用 {@link #FADE_IN_STEP_PER_TICK}。
     */
    @Deprecated
    static final float FADE_IN_STEP = FADE_IN_STEP_PER_TICK;

    /**
     * @deprecated 仅供旧测试引用。请使用 {@link #FADE_OUT_STEP_PER_TICK}。
     */
    @Deprecated
    static final float FADE_OUT_STEP = FADE_OUT_STEP_PER_TICK;

    // ──────────────────────────────────────────────────────────────────────────
    // 视野旋转参数
    // ──────────────────────────────────────────────────────────────────────────

    /**
     * 视野旋转最大幅度（±3° yaw），单位：度。
     * sin wave period 约 40 tick（2π / (2π/40) = 40）。
     */
    static final float MAX_YAW_DEGREES = 3.0f;

    /**
     * Sin wave 每 tick 相位增量（period = 40 tick）。
     * phase_increment = 2π / 40 ≈ 0.15708 rad/tick。
     * 由 {@link HallucinationTickController} 每游戏 tick 调用，与帧率无关。
     */
    static final float SIN_PHASE_INCREMENT_PER_TICK = (float) (2 * Math.PI / 40.0);

    /**
     * @deprecated 仅供旧测试引用。请使用 {@link #SIN_PHASE_INCREMENT_PER_TICK}。
     */
    @Deprecated
    static final float SIN_PHASE_INCREMENT = SIN_PHASE_INCREMENT_PER_TICK;

    // ──────────────────────────────────────────────────────────────────────────
    // Bar 偏移参数
    // ──────────────────────────────────────────────────────────────────────────

    /**
     * HP/qi bar 最大显示偏移幅度（±20%）。
     * 偏移 = random.nextFloat() * 0.4f - 0.2f（范围 -0.2 ~ +0.2）。
     */
    static final float MAX_BAR_OFFSET = 0.2f;

    /** Bar 偏移重随机间隔（每 10 tick）。 */
    static final int BAR_OFFSET_RESHUFFLE_INTERVAL_TICKS = 10;

    // ──────────────────────────────────────────────────────────────────────────
    // 边缘像差宽度
    // ──────────────────────────────────────────────────────────────────────────

    /** 边缘像差条宽（像素），绿色 fillGradient 的渐变宽度。 */
    static final int EDGE_ABERRATION_WIDTH = 32;

    private HallucinationHudOverlay() {}

    // ──────────────────────────────────────────────────────────────────────────
    // 主渲染入口
    // ──────────────────────────────────────────────────────────────────────────

    /**
     * 每帧渲染调用（由 BongHud 在 visibility == FULL 分支内调用）。
     *
     * <p><b>重要</b>：本方法只<em>读取</em> {@link HallucinationLayerStore} 状态，不递减任何计数器。
     * 所有 tick-based 状态更新（decrementTick / tickFade / bar 偏移 / 音效）均由
     * {@link HallucinationTickController} 在 {@code ClientTickEvents.END_CLIENT_TICK} 中处理。
     *
     * <p>渲染步骤：
     * <ol>
     *   <li>读取当前 fadeProgress（由 HallucinationTickController 每 tick 更新）</li>
     *   <li>绿色边缘像差 fillGradient（按 fadeProgress 调整 alpha）</li>
     *   <li>（视野旋转由调用方通过 getYawOffset() 应用，此处不直接写相机）</li>
     * </ol>
     *
     * @param context DrawContext（Fabric 1.20.1）
     */
    public static void render(DrawContext context) {
        float fade = HallucinationLayerStore.getFadeProgress();
        if (fade <= 0.0f) {
            return; // 完全不可见，跳过渲染
        }

        // 绿色边缘像差
        int sw = context.getScaledWindowWidth();
        int sh = context.getScaledWindowHeight();
        int fadedColor = applyFadeAlpha(HALLUCINATION_EDGE_COLOR_MAX, fade);
        renderEdgeAberration(context, sw, sh, fadedColor);
    }

    /**
     * 仅供测试：接受注入状态的渲染方法（绕过 MinecraftClient）。
     *
     * @param context     DrawContext
     * @param fadeProgress 注入的 fade 进度（0~1）
     */
    static void renderWithState(DrawContext context, float fadeProgress) {
        if (fadeProgress <= 0.0f) {
            return;
        }
        int sw = context.getScaledWindowWidth();
        int sh = context.getScaledWindowHeight();
        int fadedColor = applyFadeAlpha(HALLUCINATION_EDGE_COLOR_MAX, fadeProgress);
        renderEdgeAberration(context, sw, sh, fadedColor);
    }

    // ──────────────────────────────────────────────────────────────────────────
    // 视野偏移 API（供 mixin 调用）
    // ──────────────────────────────────────────────────────────────────────────

    /**
     * 返回当前帧的视野偏航偏移量（度数，范围 -MAX_YAW_DEGREES ~ +MAX_YAW_DEGREES）。
     *
     * <p>公式：{@code yaw = sin(sinPhase) × MAX_YAW_DEGREES × fadeProgress}。
     * fade 未激活时返回 0（不影响正常游戏）。
     *
     * <p>视觉观感（是否有明显旋转感）需用户 WSLg 手测验收。
     */
    public static float getYawOffset() {
        float fade = HallucinationLayerStore.getFadeProgress();
        if (fade <= 0.0f) {
            return 0.0f;
        }
        return (float) Math.sin(HallucinationLayerStore.getSinPhase()) * MAX_YAW_DEGREES * fade;
    }

    // ──────────────────────────────────────────────────────────────────────────
    // Bar 偏移 API（供 HUD bar 渲染使用）
    // ──────────────────────────────────────────────────────────────────────────

    /**
     * 返回 HP bar 显示值的偏移系数（-0.2 ~ +0.2）。
     *
     * <p>使用方式：{@code displayHp = actualHp * (1.0 + getHpBarDisplayOffset())}。
     * <strong>不得</strong>写回玩家实际 HP。
     */
    public static float getHpBarDisplayOffset() {
        float fade = HallucinationLayerStore.getFadeProgress();
        if (fade <= 0.0f) return 0.0f;
        return HallucinationLayerStore.getHpBarDisplayOffset() * fade;
    }

    /**
     * 返回 qi bar 显示值的偏移系数（-0.2 ~ +0.2）。
     *
     * <p>使用方式：{@code displayQi = actualQi * (1.0 + getQiBarDisplayOffset())}。
     * <strong>不得</strong>写回玩家实际 qi_current。
     */
    public static float getQiBarDisplayOffset() {
        float fade = HallucinationLayerStore.getFadeProgress();
        if (fade <= 0.0f) return 0.0f;
        return HallucinationLayerStore.getQiBarDisplayOffset() * fade;
    }

    // ──────────────────────────────────────────────────────────────────────────
    // 内部工具方法
    // ──────────────────────────────────────────────────────────────────────────

    /**
     * 按 fadeProgress（0~1）对 ARGB 颜色的 alpha 通道做线性插值。
     *
     * @param argb         原始 ARGB 颜色（高8位为目标 alpha）
     * @param fadeProgress 0.0f（透明）到 1.0f（完整 alpha）
     * @return 插值后的 ARGB 颜色
     */
    static int applyFadeAlpha(int argb, float fadeProgress) {
        int targetAlpha = (argb >>> 24) & 0xFF;
        int fadedAlpha = Math.round(targetAlpha * fadeProgress);
        return (fadedAlpha << 24) | (argb & 0x00FFFFFF);
    }

    /**
     * 渲染四边缘绿色像差条（fillGradient 近似晕染效果）。
     *
     * @param context     DrawContext
     * @param screenWidth  屏幕宽度
     * @param screenHeight 屏幕高度
     * @param color        插值后的 ARGB 颜色
     */
    private static void renderEdgeAberration(
            DrawContext context,
            int screenWidth,
            int screenHeight,
            int color
    ) {
        int w = EDGE_ABERRATION_WIDTH;
        // 上边缘
        context.fillGradient(0, 0, screenWidth, w, color, TRANSPARENT);
        // 下边缘
        context.fillGradient(0, screenHeight - w, screenWidth, screenHeight, TRANSPARENT, color);
        // 左边缘
        context.fillGradient(0, 0, w, screenHeight, color, TRANSPARENT);
        // 右边缘
        context.fillGradient(screenWidth - w, 0, screenWidth, screenHeight, TRANSPARENT, color);
    }

}

