package com.bong.client.visual;

import net.minecraft.client.gui.DrawContext;

/**
 * 边缘 vignette 渲染。对 `HudRenderCommand.EDGE_VIGNETTE` 的渲染端实现。
 *
 * 做法：
 * - 顶/底用 `fillGradient` 天然的纵向渐变（MC 1.20.1 `DrawContext.fillGradient`
 *   是从 `colorStart` 到 `colorEnd` 的纵向插值）。
 * - 左/右用若干垂直细条模拟横向渐变（MC vanilla 的 DrawContext 不直接提供横向
 *   渐变 API，用 N 条 `fill` 的 alpha 阶梯近似）。
 *
 * 中心透明化：任意 RGB 配 alpha=0 即视为透明。
 */
public final class EdgeDecalRenderer {
    /** 顶/底 vignette 深入屏幕的比例（默认 25%）。 */
    static final double VERTICAL_THICKNESS_RATIO = 0.25;
    /** 左/右 vignette 深入屏幕的比例（默认 15%）。 */
    static final double HORIZONTAL_THICKNESS_RATIO = 0.15;
    /** 左/右用阶梯条数近似渐变，越多越平滑但开销也越大。 */
    static final int SIDE_STEP_BANDS = 16;

    private EdgeDecalRenderer() {
    }

    public static void render(DrawContext context, int screenWidth, int screenHeight, int argbColor) {
        if (context == null || screenWidth <= 0 || screenHeight <= 0) {
            return;
        }
        int baseAlpha = (argbColor >>> 24) & 0xFF;
        if (baseAlpha == 0) {
            return;
        }
        int rgb = argbColor & 0x00FFFFFF;
        int transparent = rgb; // alpha=0

        int verticalThickness = Math.max(1, (int) Math.round(screenHeight * VERTICAL_THICKNESS_RATIO));
        int horizontalThickness = Math.max(1, (int) Math.round(screenWidth * HORIZONTAL_THICKNESS_RATIO));

        // Top: 顶边满色 → 中心方向透明
        context.fillGradient(0, 0, screenWidth, verticalThickness, argbColor, transparent);
        // Bottom: 中心方向透明 → 底边满色
        context.fillGradient(0, screenHeight - verticalThickness, screenWidth, screenHeight, transparent, argbColor);

        // Left / Right 横向渐变近似
        int bandCount = Math.max(1, SIDE_STEP_BANDS);
        int bandWidth = Math.max(1, horizontalThickness / bandCount);
        for (int i = 0; i < bandCount; i++) {
            double t = (double) i / bandCount; // 0 = 紧贴屏幕边（满色），1 = 最靠中心（透明）
            int bandAlpha = Math.max(0, (int) Math.round(baseAlpha * (1.0 - t)));
            if (bandAlpha == 0) {
                continue;
            }
            int bandColor = (bandAlpha << 24) | rgb;
            int leftX = i * bandWidth;
            context.fill(leftX, 0, leftX + bandWidth, screenHeight, bandColor);
            int rightXEnd = screenWidth - leftX;
            context.fill(rightXEnd - bandWidth, 0, rightXEnd, screenHeight, bandColor);
        }
    }
}
