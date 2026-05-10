package com.bong.client.inventory.component;

import com.bong.client.inventory.model.InventoryItem;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ItemTooltipPanelTest {
    @Test
    void spiritQualityLabelAndBarClampToTooltipWidth() {
        InventoryItem item = InventoryItem.createFull(
            7L,
            "kaimai_dan",
            "开脉丹",
            1,
            1,
            0.2,
            "rare",
            "",
            1,
            0.72,
            1.0
        );

        assertEquals("灵质 72%", ItemTooltipPanel.spiritQualityLabel(item));
        assertEquals(141, ItemTooltipPanel.qualityBarFillWidth(196, item.spiritQuality()));
        assertEquals(0, ItemTooltipPanel.qualityBarFillWidth(196, -1.0));
        assertEquals(196, ItemTooltipPanel.qualityBarFillWidth(196, 2.0));
    }

    @Test
    void qualityBarColorMovesFromGreyThroughGreenToGold() {
        assertEquals(0x888888, ItemTooltipPanel.qualityBarColor(0.0));
        assertEquals(0x22CC22, ItemTooltipPanel.qualityBarColor(0.5));
        assertEquals(0xFFAA00, ItemTooltipPanel.qualityBarColor(1.0));
    }

    @Test
    void ancientRelicStatusIncludesChargesWarning() {
        InventoryItem relic = InventoryItem.createFullWithVisualMeta(
            77L,
            "ancient_broken_blade",
            "上古断刃",
            1,
            2,
            1.0,
            "ancient",
            "",
            1,
            0.0,
            1.0,
            3,
            "",
            "",
            0,
            null,
            "",
            java.util.List.of(),
            null,
            java.util.List.of()
        );

        String status = ItemTooltipPanel.formatStatusLine(relic);
        assertTrue(status.contains("⚡ ×3"));
        assertTrue(status.contains("上古遗物·一次性"));
    }

    @Test
    void mundaneArmorTooltipLinesShowMaterialDefenseAndRepairHint() {
        InventoryItem armor = InventoryItem.createFull(
            88L,
            "armor_copper_chestplate",
            "铜甲胸甲",
            2,
            2,
            2.1,
            "common",
            "",
            1,
            1.0,
            0.0
        );

        assertEquals("凡物·铜制", ItemTooltipPanel.armorMaterialLine(armor));
        assertEquals("防御: +2.80", ItemTooltipPanel.armorDefenseLine(armor));
        assertEquals("已损坏·不可穿戴", ItemTooltipPanel.armorBrokenLine(armor));
        assertEquals("修复: 同材质 ×2 hand-craft", ItemTooltipPanel.armorRepairLine(armor));
    }

    @Test
    void rarityLabelsCoverAllSixTiers() {
        assertRarityLabelAndColor("common", "普通", 0x808080);
        assertRarityLabelAndColor("uncommon", "精良", 0x22CC22);
        assertRarityLabelAndColor("rare", "稀有", 0x2288FF);
        assertRarityLabelAndColor("epic", "史诗", 0xAA44FF);
        assertRarityLabelAndColor("legendary", "传说", 0xFFAA00);
        assertRarityLabelAndColor(" Ancient ", "上古", 0xFF4444);
    }

    private static void assertRarityLabelAndColor(String rarity, String label, int color) {
        InventoryItem item = InventoryItem.createFull(
            99L,
            "fixture",
            "fixture",
            1,
            1,
            0.1,
            rarity,
            "",
            1,
            1.0,
            1.0
        );

        assertEquals(label, ItemTooltipPanel.rarityLabel(rarity));
        assertEquals(color, item.rarityColor());
    }
}
