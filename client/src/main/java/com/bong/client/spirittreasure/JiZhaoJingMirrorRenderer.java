package com.bong.client.spirittreasure;

import net.minecraft.client.gui.DrawContext;

public final class JiZhaoJingMirrorRenderer {
    private JiZhaoJingMirrorRenderer() {
    }

    public static int colorForAffinity(double affinity) {
        double clamped = Math.max(0.0, Math.min(1.0, affinity));
        int cold = (int) Math.round(90 + clamped * 70);
        int light = (int) Math.round(120 + clamped * 95);
        return 0xFF000000 | (cold << 16) | (light << 8) | 210;
    }

    public static void render(DrawContext context, int x, int y, int size, double affinity, long nowMs) {
        int border = colorForAffinity(affinity);
        int inner = 0xDD101A22;
        context.fill(x, y, x + size, y + size, 0xAA05070A);
        context.drawBorder(x, y, size, size, border);
        context.fill(x + 4, y + 4, x + size - 4, y + size - 4, inner);

        int phase = (int) ((nowMs / 140L) % Math.max(1, size / 2));
        for (int i = 0; i < 3; i++) {
            int inset = 8 + ((phase + i * 11) % Math.max(1, size / 2 - 8));
            int alpha = 0x33000000 - i * 0x08000000;
            context.drawBorder(
                x + inset,
                y + inset,
                Math.max(4, size - inset * 2),
                Math.max(4, size - inset * 2),
                alpha | (border & 0x00FFFFFF)
            );
        }
    }
}
