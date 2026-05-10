package com.bong.client.inventory;

import com.bong.client.botany.BotanySpiritQualityVisuals;
import com.bong.client.inventory.model.InventoryItem;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

public class BotanySpiritQualityVisualsTest {
    @Test
    void spirit_quality_border_renders() {
        InventoryItem common = item("ning_mai_cao", 0.49);
        InventoryItem uncommon = item("ning_mai_cao", 0.50);
        InventoryItem rare = item("ning_mai_cao", 0.70);
        InventoryItem epic = item("ning_mai_cao", 0.90);
        InventoryItem nonPlant = item("iron_sword", 0.95);

        assertEquals(BotanySpiritQualityVisuals.NO_BORDER, BotanySpiritQualityVisuals.borderColor(common));
        assertEquals(BotanySpiritQualityVisuals.UNCOMMON_BORDER, BotanySpiritQualityVisuals.borderColor(uncommon));
        assertEquals(BotanySpiritQualityVisuals.RARE_BORDER, BotanySpiritQualityVisuals.borderColor(rare));
        assertEquals(BotanySpiritQualityVisuals.EPIC_BORDER, BotanySpiritQualityVisuals.borderColor(epic));
        assertEquals(BotanySpiritQualityVisuals.NO_BORDER, BotanySpiritQualityVisuals.borderColor(nonPlant));
        assertEquals("品质 90%", BotanySpiritQualityVisuals.qualityLabel(epic));
    }

    private static InventoryItem item(String itemId, double spiritQuality) {
        return InventoryItem.createFull(
            1L,
            itemId,
            itemId,
            1,
            1,
            0.2,
            "common",
            "",
            1,
            spiritQuality,
            1.0
        );
    }
}
