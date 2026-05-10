package com.bong.client.inventory;

import com.bong.client.inventory.model.InventoryItem;
import net.minecraft.client.gui.DrawContext;

public final class RarityBorderRenderer {
    private static final float ANCIENT_PULSE_PERIOD_TICKS = 40.0f;
    private static final float ANCIENT_FLASH_PERIOD_TICKS = 60.0f;
    private static final float ANCIENT_FLASH_DURATION_TICKS = 4.0f;

    private RarityBorderRenderer() {}

    public static int colorRgb(String rarity) {
        return switch (normalize(rarity)) {
            case "uncommon" -> 0x22CC22;
            case "rare" -> 0x2288FF;
            case "epic" -> 0xAA44FF;
            case "legendary" -> 0xFFAA00;
            case "ancient" -> 0xFF4444;
            default -> 0x808080;
        };
    }

    public static int colorArgb(String rarity, float ageTicks) {
        int alpha = "ancient".equals(normalize(rarity)) ? ancientPulseAlpha(ageTicks) : 0xCC;
        return (alpha << 24) | colorRgb(rarity);
    }

    public static int ancientPulseAlpha(float ageTicks) {
        double phase = Math.sin((ageTicks / ANCIENT_PULSE_PERIOD_TICKS) * Math.PI * 2.0);
        return 0x80 + (int) Math.round(((phase + 1.0) * 0.5) * 0x7F);
    }

    public static int ancientInvertFlashAlpha(float ageTicks) {
        float phase = positiveModulo(ageTicks, ANCIENT_FLASH_PERIOD_TICKS);
        if (phase >= ANCIENT_FLASH_DURATION_TICKS) {
            return 0;
        }
        double fade = 1.0 - (phase / ANCIENT_FLASH_DURATION_TICKS);
        return (int) Math.round(0x66 * fade);
    }

    public static void drawBorder(DrawContext context, int x, int y, int w, int h, InventoryItem item, float ageTicks) {
        if (context == null || item == null || item.isEmpty()) return;

        int color = colorArgb(item.rarity(), ageTicks);
        context.fill(x, y, x + w, y + 1, color);
        context.fill(x, y + h - 1, x + w, y + h, color);
        context.fill(x, y + 1, x + 1, y + h - 1, color);
        context.fill(x + w - 1, y + 1, x + w, y + h - 1, color);
    }

    private static String normalize(String rarity) {
        return rarity == null ? "common" : rarity.trim().toLowerCase(java.util.Locale.ROOT);
    }

    private static float positiveModulo(float value, float divisor) {
        float result = value % divisor;
        return result < 0 ? result + divisor : result;
    }
}
