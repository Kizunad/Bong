package com.bong.client.alchemy;

import io.wispforest.owo.ui.core.OwoUIDrawContext;
import net.minecraft.util.Identifier;

/** 炉边纸材：丹方用卷轴，炉记与部位小笺用旧纸，边饰不随窗口比例拉伸。 */
final class AlchemyPaperSurface {
    static final Identifier RECIPE = texture("recipe-paper.png");
    static final Identifier JOURNAL = texture("journal-paper.png");

    private AlchemyPaperSurface() {}

    private static Identifier texture(String file) {
        return new Identifier("bong-client", "textures/gui/alchemy/" + file);
    }

    static void draw(OwoUIDrawContext context, Identifier texture, int x, int y, int width, int height, int edge) {
        // 原图为 512×768。端头、纸边和装订独立缩放，只有纸心延展。
        edge = Math.max(0, Math.min(edge, Math.min(width, height) / 2));
        int[] sourceX = {0, 56, 456, 512};
        int[] sourceY = {0, 56, 712, 768};
        int[] targetX = {x, x + edge, x + width - edge, x + width};
        int[] targetY = {y, y + edge, y + height - edge, y + height};
        for (int row = 0; row < 3; row++) {
            for (int col = 0; col < 3; col++) {
                context.drawTexture(texture, targetX[col], targetY[row],
                    targetX[col + 1] - targetX[col], targetY[row + 1] - targetY[row],
                    sourceX[col], sourceY[row], sourceX[col + 1] - sourceX[col],
                    sourceY[row + 1] - sourceY[row], 512, 768);
            }
        }
    }
}
