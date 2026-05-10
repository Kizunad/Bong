package com.bong.client.inventory;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.component.ItemTooltipPanel;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class InventoryItemTest {

    @Test
    void legacyFactoryDefaultsAuthoritativeFields() {
        InventoryItem item = InventoryItem.create(
            "starter_talisman",
            "Starter Talisman",
            1,
            1,
            0.5,
            "common",
            ""
        );

        assertEquals(0L, item.instanceId());
        assertEquals(1, item.stackCount());
        assertEquals(1.0, item.spiritQuality(), 1e-9);
        assertEquals(1.0, item.durability(), 1e-9);
    }

    @Test
    void fullFactoryClampsNewInventoryFields() {
        InventoryItem item = InventoryItem.createFull(
            42L,
            "weathered_pill",
            "Weathered Pill",
            1,
            1,
            0.2,
            "rare",
            "",
            0,
            -0.25,
            1.75
        );

        assertEquals(42L, item.instanceId());
        assertEquals(1, item.stackCount());
        assertEquals(0.0, item.spiritQuality(), 1e-9);
        assertEquals(1.0, item.durability(), 1e-9);
    }

    @Test
    void nanQualityAndDurabilityDefaultToFullValue() {
        InventoryItem item = InventoryItem.createFull(
            43L,
            "unstable_pill",
            "Unstable Pill",
            1,
            1,
            0.2,
            "rare",
            "",
            2,
            Double.NaN,
            Double.NaN
        );

        assertEquals(1.0, item.spiritQuality(), 1e-9);
        assertEquals(1.0, item.durability(), 1e-9);
    }

    @Test
    void boneCoinTooltipUsesSealedQiSemantics() {
        InventoryItem item = InventoryItem.createFull(
            44L,
            "bone_coin_15",
            "封灵骨币",
            1,
            1,
            0.1,
            "common",
            "",
            1,
            0.42,
            1.0
        );

        assertTrue(item.isBoneCoin());
        assertEquals("封灵真元 42%", ItemTooltipPanel.formatStatusLine(item));
    }

    @Test
    void ancientRelicGlowRendersChargesInStatusLine() {
        InventoryItem item = InventoryItem.createFullWithVisualMeta(
            77L,
            "ancient_relic_eye",
            "古眼",
            1,
            1,
            0.8,
            "ancient",
            "",
            1,
            1.0,
            0.9,
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

        assertTrue(item.isAncientRelic());
        assertEquals(0xFF4444, item.rarityColor());
        assertEquals("耐久 90%  ⚡ ×3 上古遗物·一次性", ItemTooltipPanel.formatStatusLine(item));
        assertTrue(AncientRelicGlowRenderer.shouldGlow(item));
    }

    @Test
    void ancientRelicChargesClampAndMarkRelic() {
        InventoryItem item = InventoryItem.createFullWithVisualMeta(
            45L,
            "ancient_relic",
            "上古遗物",
            1,
            1,
            0.2,
            "ancient",
            "",
            1,
            0.0,
            1.0,
            8,
            "",
            "",
            0,
            null,
            "",
            java.util.List.of(),
            null,
            java.util.List.of()
        );

        assertTrue(item.isAncientRelic());
        assertEquals(5, item.charges());
        assertEquals(0xFF4444, item.rarityColor());
    }

    @Test
    void rarityMetadataIsNormalizedForVisualSemantics() {
        InventoryItem item = InventoryItem.createFullWithVisualMeta(
            46L,
            "ancient_relic",
            "上古遗物",
            1,
            1,
            0.2,
            " Ancient ",
            "",
            1,
            1.0,
            1.0,
            1,
            "",
            "",
            0,
            null,
            "",
            java.util.List.of(),
            null,
            java.util.List.of()
        );

        assertEquals("ancient", item.rarity());
        assertTrue(item.isAncientRelic());
        assertEquals(0xFF4444, item.rarityColor());
    }
}
