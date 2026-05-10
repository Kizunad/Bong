package com.bong.client.botany;

import com.bong.client.inventory.model.InventoryItem;

import java.util.Locale;

public final class BotanySpiritQualityVisuals {
    public static final int NO_BORDER = 0;
    public static final int UNCOMMON_BORDER = 0xCC22FF44;
    public static final int RARE_BORDER = 0xCC4AA3FF;
    public static final int EPIC_BORDER = 0xCCD060FF;

    private BotanySpiritQualityVisuals() {}

    public static boolean isBotanyPlant(InventoryItem item) {
        return item != null && BotanyPlantItemIds.contains(item.itemId());
    }

    public static int borderColor(InventoryItem item) {
        if (!isBotanyPlant(item)) {
            return NO_BORDER;
        }
        double q = item.spiritQuality();
        if (q >= 0.90) {
            return EPIC_BORDER;
        }
        if (q >= 0.70) {
            return RARE_BORDER;
        }
        if (q >= 0.50) {
            return UNCOMMON_BORDER;
        }
        return NO_BORDER;
    }

    public static int barColor(InventoryItem item) {
        int border = borderColor(item);
        return border == NO_BORDER ? 0xFF88CC88 : (0xFF000000 | (border & 0x00FFFFFF));
    }

    public static String qualityLabel(InventoryItem item) {
        if (!isBotanyPlant(item)) {
            return "";
        }
        return String.format(Locale.ROOT, "品质 %.0f%%", item.spiritQuality() * 100.0);
    }
}
