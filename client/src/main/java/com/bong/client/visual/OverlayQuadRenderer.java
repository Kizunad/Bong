package com.bong.client.visual;

import net.minecraft.client.gui.DrawContext;

/**
 * 全屏半透明叠层。对 `HudRenderCommand.SCREEN_TINT` 的渲染端实现。
 * color 为 ARGB（HudTextHelper.withAlpha 约定）。
 */
public final class OverlayQuadRenderer {
    private OverlayQuadRenderer() {
    }

    public static void render(DrawContext context, int screenWidth, int screenHeight, int argbColor) {
        if (context == null || screenWidth <= 0 || screenHeight <= 0) {
            return;
        }
        if (alphaOf(argbColor) == 0) {
            return;
        }
        context.fill(0, 0, screenWidth, screenHeight, argbColor);
    }

    static int alphaOf(int argb) {
        return (argb >>> 24) & 0xFF;
    }
}
