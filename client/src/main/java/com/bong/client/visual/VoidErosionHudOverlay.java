package com.bong.client.visual;

import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.font.TextRenderer;

/**
 * plan-combat-skill-feedback-bridges-v1 P3 — 虚蚀视觉 HUD overlay。
 *
 * 阶段 ≥ 3（EchoBody）时显示声音扭曲 overlay（紫色边缘晕染）
 * 并在左上角展示虚蚀阶段文字。
 *
 * 渲染接入点：{@link com.bong.client.BongHud} 里 {@code visibility == FULL} 分支。
 */
public final class VoidErosionHudOverlay {

    /** 声音扭曲 overlay 颜色（阶段 3+）：紫色半透明。 */
    public static final int DISTORTION_VIGNETTE_COLOR = 0x553322AA;
    /** 阶段 4 深度扭曲颜色：更深紫。 */
    public static final int DISTORTION_VIGNETTE_STAGE4_COLOR = 0x884422CC;

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

    private VoidErosionHudOverlay() {}

    /**
     * 每帧渲染调用（由 BongHud 主线程驱动）。
     *
     * @param context  DrawContext（Fabric 1.20.1）
     * @param renderer 字体渲染器
     */
    public static void render(DrawContext context, TextRenderer renderer) {
        VoidErosionVisualStore.State state = VoidErosionVisualStore.snapshot();
        if (state == null || state.stage() == 0) {
            return;
        }

        int stage = state.stage();

        // 声音扭曲 overlay（阶段 3+）：紫色边缘晕染
        if (state.soundDistortionActive()) {
            int vignetteColor = (stage >= 4) ? DISTORTION_VIGNETTE_STAGE4_COLOR : DISTORTION_VIGNETTE_COLOR;
            int sw = context.getScaledWindowWidth();
            int sh = context.getScaledWindowHeight();
            // 渲染半透明 quad（使用 fillGradient 近似边缘晕效果）
            renderDistortionOverlay(context, sw, sh, vignetteColor);
        }

        // 阶段文字（左上角）
        String label = (stage >= 0 && stage < STAGE_LABELS.length) ? STAGE_LABELS[stage] : "";
        if (!label.isEmpty()) {
            int textColor = (stage >= 0 && stage < STAGE_TEXT_COLORS.length)
                    ? STAGE_TEXT_COLORS[stage]
                    : 0xFFFFFFFF;
            context.drawText(renderer, label, OVERLAY_X, OVERLAY_Y_BASE, textColor, true);
        }
    }

    private static void renderDistortionOverlay(
            DrawContext context,
            int screenWidth,
            int screenHeight,
            int color
    ) {
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
