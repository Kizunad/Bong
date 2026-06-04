package com.bong.client.visual;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.font.TextRenderer;

/**
 * plan-combat-skill-feedback-bridges-v1 P3 fix4/fix5 — 虚蚀视觉 HUD overlay。
 *
 * <p>阶段 ≥ 3（EchoBody）时显示「虚蚀扭曲」overlay（紫色边缘晕染，渐变 fade 曲线）
 * 并在左上角展示虚蚀阶段文字。
 *
 * <p><b>fix4</b>：vignette 改为按 fade 曲线 lerp——进入时 alpha 从 0 逐帧递增到目标值，
 * 退出时从当前 alpha 逐帧衰减到 0，而非原来的二值切换。每帧 60fps 下约 0.5s 完成过渡。
 *
 * <p><b>fix5</b>：字段 {@code soundDistortionActive} 诚实重命名为
 * {@code voidDistortionActive}——该字段仅控制视觉扭曲 overlay，无音频逻辑。
 * 虚蚀阶段音效通过独立的 {@code bong:audio/play} channel（{@code PlaySoundRecipeRequest}）
 * 触发，不经本 overlay。
 * §视听精度文档待人工补（consume 约束#5禁改 docs/ 非 plan 文件）。
 *
 * <p>渲染接入点：{@link com.bong.client.BongHud} 里 {@code visibility == FULL} 分支。
 */
public final class VoidErosionHudOverlay {

    // ──────────────────────────────────────────────────────────────
    // 颜色常量（命名遵循 stage 层次；ARGB 格式，高8位 = alpha 0x00-0xFF）
    // ──────────────────────────────────────────────────────────────

    /**
     * 虚蚀扭曲 overlay 基础颜色（阶段 3 · EchoBody）。
     * ARGB = 0x55_33_22_AA：alpha=0x55(约33%)，RGB=(0x33,0x22,0xAA)深紫蓝色。
     * 受影响阶段：stage == 3。
     */
    static final int VOID_DISTORTION_VIGNETTE_STAGE3_COLOR = 0x553322AA;

    /**
     * 虚蚀扭曲 overlay 深度颜色（阶段 4 · VoidEroded）。
     * ARGB = 0x88_44_22_CC：alpha=0x88(约53%)，RGB=(0x44,0x22,0xCC)更深紫色。
     * 受影响阶段：stage >= 4，表达玩家已完全虚蚀。
     */
    static final int VOID_DISTORTION_VIGNETTE_STAGE4_COLOR = 0x884422CC;

    // ──────────────────────────────────────────────────────────────
    // Fade 曲线参数（fix4：进入/退出平滑过渡，约 0.5s @ 60fps）
    // ──────────────────────────────────────────────────────────────

    /** fade-in 每帧递增量（0~1 范围，30帧内完成 = 0.5s @ 60fps）。 */
    static final float FADE_IN_STEP = 1.0f / 30.0f;

    /** fade-out 每帧衰减量（同速率，保持对称过渡）。 */
    static final float FADE_OUT_STEP = 1.0f / 30.0f;

    // ──────────────────────────────────────────────────────────────
    // 阶段文字映射
    // ──────────────────────────────────────────────────────────────

    /** 阶段文字颜色映射（stage → ARGB）。 */
    private static final int[] STAGE_TEXT_COLORS = {
        0xFFAAAAAA, // 0 — None
        0xFF88BBFF, // 1 — LowPressure（淡蓝）
        0xFF5566EE, // 2 — VoidShadow（深蓝紫）
        0xFF9922CC, // 3 — EchoBody（紫）
        0xFFCC2288, // 4 — VoidEroded（洋红）
    };

    private static final String[] STAGE_LABELS = {
        "",
        "§9虚蚀·低压",
        "§5虚蚀·虚影",
        "§5虚蚀·回响体",
        "§d虚蚀·虚蚀态",
    };

    private static final int OVERLAY_X = 10;
    private static final int OVERLAY_Y_BASE = 40;

    // ──────────────────────────────────────────────────────────────
    // fade 状态（跨帧，范围 0.0f–1.0f）
    // ──────────────────────────────────────────────────────────────

    /** 当前 vignette alpha 进度（0.0 = 不可见，1.0 = 完全目标 alpha）。 */
    private static float fadeProgress = 0.0f;

    private VoidErosionHudOverlay() {}

    /**
     * 每帧渲染调用（由 BongHud 主线程驱动）。
     *
     * <p>fix4：vignette alpha 通过 {@code fadeProgress} lerp，
     * 进入时 fade-in，退出时 fade-out，而非二值切换。
     *
     * <p>fix-per-entity：通过本地玩家名构造 wire ID（"offline:{name}"），
     * 调用 {@link VoidErosionVisualStore#snapshotForEntity(String)} 只取
     * 本机玩家的虚蚀状态，多人场景下其他玩家的虚蚀不串扰本地 HUD vignette。
     *
     * @param context  DrawContext（Fabric 1.20.1）
     * @param renderer 字体渲染器
     */
    public static void render(DrawContext context, TextRenderer renderer) {
        // 取本机玩家 wire ID，与 server emit 的 "offline:{username}" 对齐
        MinecraftClient mc = MinecraftClient.getInstance();
        String localWireId = (mc.player != null)
                ? "offline:" + mc.player.getEntityName()
                : null;
        VoidErosionVisualStore.State state = (localWireId != null)
                ? VoidErosionVisualStore.snapshotForEntity(localWireId)
                : null;
        renderWithState(context, renderer, state);
    }

    /**
     * 核心渲染逻辑，接受已解析的 per-entity State（可为 null）。
     *
     * <p>从 {@link #render} 中提取，方便单元测试直接注入 state，
     * 绕过 {@link MinecraftClient#getInstance()}（测试环境无 MC 上下文）。
     *
     * <p><b>多人 HUD 只反映本机玩家虚蚀：</b> 调用方负责按本地玩家 wire ID
     * 查 {@link VoidErosionVisualStore#snapshotForEntity} 后传入，其他玩家
     * 的 erosion state 永远不应被传入此方法作为 "local state"。
     *
     * @param context  DrawContext（Fabric 1.20.1）
     * @param renderer 字体渲染器
     * @param state    本机玩家的虚蚀状态快照，或 null（无虚蚀时）
     */
    static void renderWithState(DrawContext context, TextRenderer renderer, VoidErosionVisualStore.State state) {
        int stage = (state != null) ? state.stage() : 0;
        // fix5: voidDistortionActive（原 soundDistortionActive）仅控制视觉扭曲 overlay
        boolean voidDistortionActive = (state != null) && state.voidDistortionActive();

        // ── fade 曲线更新 ────────────────────────────────────────────
        if (voidDistortionActive) {
            fadeProgress = Math.min(1.0f, fadeProgress + FADE_IN_STEP);
        } else {
            fadeProgress = Math.max(0.0f, fadeProgress - FADE_OUT_STEP);
        }

        // ── vignette 渲染（仅在 fadeProgress > 0 时绘制）───────────
        if (fadeProgress > 0.0f && stage > 0) {
            // 按阶段选目标颜色，再按 fadeProgress 调整 alpha
            int baseColor = (stage >= 4)
                    ? VOID_DISTORTION_VIGNETTE_STAGE4_COLOR
                    : VOID_DISTORTION_VIGNETTE_STAGE3_COLOR;
            int fadedColor = applyFadeAlpha(baseColor, fadeProgress);

            int sw = context.getScaledWindowWidth();
            int sh = context.getScaledWindowHeight();
            renderDistortionVignette(context, sw, sh, fadedColor);
        }

        // ── 阶段文字（左上角，只有 stage > 0 才显示）───────────────
        if (state == null || stage == 0) {
            return;
        }
        String label = (stage >= 0 && stage < STAGE_LABELS.length) ? STAGE_LABELS[stage] : "";
        if (!label.isEmpty()) {
            int textColor = (stage >= 0 && stage < STAGE_TEXT_COLORS.length)
                    ? STAGE_TEXT_COLORS[stage]
                    : 0xFFFFFFFF;
            context.drawText(renderer, label, OVERLAY_X, OVERLAY_Y_BASE, textColor, true);
        }
    }

    /**
     * 按 {@code fadeProgress}（0~1）对 ARGB 颜色的 alpha 通道做线性插值。
     *
     * <p>入参 {@code argb} 的高 8 位为目标 alpha；输出高 8 位 = target_alpha × fadeProgress。
     *
     * @param argb         原始 ARGB 颜色（高8位为最大 alpha）
     * @param fadeProgress 0.0f（透明）到 1.0f（完整 alpha）
     * @return 插值后的 ARGB 颜色
     */
    static int applyFadeAlpha(int argb, float fadeProgress) {
        int targetAlpha = (argb >>> 24) & 0xFF;
        int fadedAlpha = Math.round(targetAlpha * fadeProgress);
        return (fadedAlpha << 24) | (argb & 0x00FFFFFF);
    }

    /** 仅供测试：重置 fade 状态。 */
    static void resetFadeForTest() {
        fadeProgress = 0.0f;
    }

    /** 仅供测试：注入 fade 进度值。 */
    static void setFadeProgressForTest(float value) {
        fadeProgress = Math.max(0.0f, Math.min(1.0f, value));
    }

    /** 仅供测试：读取当前 fade 进度。 */
    static float getFadeProgressForTest() {
        return fadeProgress;
    }

    private static void renderDistortionVignette(
            DrawContext context,
            int screenWidth,
            int screenHeight,
            int color
    ) {
        // 四边缘渐变（fillGradient 近似晕染效果）
        // 上边缘
        context.fillGradient(0, 0, screenWidth, 24, color, 0x00000000);
        // 下边缘
        context.fillGradient(0, screenHeight - 24, screenWidth, screenHeight, 0x00000000, color);
        // 左边缘
        context.fillGradient(0, 0, 24, screenHeight, color, 0x00000000);
        // 右边缘
        context.fillGradient(screenWidth - 24, 0, screenWidth, screenHeight, 0x00000000, color);
    }
}
