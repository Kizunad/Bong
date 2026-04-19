package com.bong.client.visual;

import com.mojang.blaze3d.systems.RenderSystem;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.util.Identifier;

/**
 * 水墨边框 overlay（plan-vfx-v1 §3.2 的"水墨边框"档）。
 *
 * 渲染 `bong-client:textures/hud/ink_wash_vignette.png`（1536×1024, 中心 alpha=0,
 * 四角墨晕）拉伸到全屏，alpha 由命令传入 color 的高字节决定 → `setShaderColor`
 * 的 a 通道控制整体淡出。PNG 已内嵌 alpha，RGB 不参与计算。
 */
public final class InkWashVignetteRenderer {
    private static final Identifier TEXTURE = new Identifier("bong-client", "textures/hud/ink_wash_vignette.png");
    private static final int TEXTURE_WIDTH = 1536;
    private static final int TEXTURE_HEIGHT = 1024;

    private InkWashVignetteRenderer() {
    }

    public static void render(DrawContext context, int screenWidth, int screenHeight, int argbColor) {
        if (context == null || screenWidth <= 0 || screenHeight <= 0) {
            return;
        }
        int baseAlpha = (argbColor >>> 24) & 0xFF;
        if (baseAlpha == 0) {
            return;
        }

        float alphaFactor = baseAlpha / 255.0f;
        RenderSystem.enableBlend();
        RenderSystem.defaultBlendFunc();
        RenderSystem.setShaderColor(1.0f, 1.0f, 1.0f, alphaFactor);

        context.drawTexture(
            TEXTURE,
            0, 0,
            screenWidth, screenHeight,
            0.0f, 0.0f,
            TEXTURE_WIDTH, TEXTURE_HEIGHT,
            TEXTURE_WIDTH, TEXTURE_HEIGHT
        );

        RenderSystem.setShaderColor(1.0f, 1.0f, 1.0f, 1.0f);
        RenderSystem.disableBlend();
    }
}
