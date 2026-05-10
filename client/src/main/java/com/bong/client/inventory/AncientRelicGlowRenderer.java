package com.bong.client.inventory;

import com.bong.client.inventory.model.InventoryItem;
import net.minecraft.client.gui.DrawContext;

/** 上古遗物在物品格、tooltip 与掉落物上的统一发光参数。 */
public final class AncientRelicGlowRenderer {
    private static final int GOLD = 0xFFD700;
    private static final int AMBER = 0xFF8800;

    private AncientRelicGlowRenderer() {
    }

    public static boolean shouldGlow(InventoryItem item) {
        return item != null && item.isAncientRelic() && !item.isEmpty();
    }

    public static String chargesLine(InventoryItem item) {
        if (item == null || item.charges() == null) {
            return "";
        }
        String line = "⚡ ×" + item.charges();
        return item.isAncientRelic() ? line + " 上古遗物·一次性" : line;
    }

    public static int pulseColor(long nowMillis) {
        double phase = (Math.sin((nowMillis % 2_000L) / 2_000.0 * Math.PI * 2.0) + 1.0) * 0.5;
        int r = 0xFF;
        int g = lerp((GOLD >> 8) & 0xFF, (AMBER >> 8) & 0xFF, phase);
        int b = lerp(GOLD & 0xFF, AMBER & 0xFF, phase);
        int alpha = 0xAA + (int) Math.round(phase * 0x44);
        return (alpha << 24) | (r << 16) | (g << 8) | b;
    }

    public static void drawGlowBorder(DrawContext context, int x, int y, int w, int h, long nowMillis) {
        int color = pulseColor(nowMillis);
        context.fill(x, y, x + w, y + 1, color);
        context.fill(x, y + h - 1, x + w, y + h, color);
        context.fill(x, y + 1, x + 1, y + h - 1, color);
        context.fill(x + w - 1, y + 1, x + w, y + h - 1, color);
    }

    private static int lerp(int a, int b, double t) {
        return (int) Math.round(a + (b - a) * t);
    }
}
