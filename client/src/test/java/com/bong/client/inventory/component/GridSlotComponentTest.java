package com.bong.client.inventory.component;

import com.bong.client.inventory.model.InventoryItem;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class GridSlotComponentTest {

    @Test
    void scrollLikeIdsUseScrollFallbackTexture() {
        assertTrue(GridSlotComponent.isScrollTextureCandidate("recipe_scroll_qixue_pill"));
        assertTrue(GridSlotComponent.isScrollTextureCandidate("skill_scroll_herbalism_baicao_can"));
        assertTrue(GridSlotComponent.isScrollTextureCandidate("blueprint_scroll_bronze_tripod"));

        assertEquals(
            new Identifier("bong-client", "textures/gui/items/broken_artifact_scroll.png"),
            GridSlotComponent.fallbackTextureIdForItemId("recipe_scroll_qixue_pill")
        );
    }

    @Test
    void nonScrollIdsUseGenericFallbackTexture() {
        assertFalse(GridSlotComponent.isScrollTextureCandidate("healing_draught"));
        assertEquals(
            new Identifier("bong-client", "textures/gui/items/broken_artifact.png"),
            GridSlotComponent.fallbackTextureIdForItemId("healing_draught")
        );
    }

    @Test
    void lowDurabilityMundaneArmorFlashesRedButBrokenArmorDoesNotPulse() {
        InventoryItem low = InventoryItem.createFull(
            1L,
            "armor_iron_chestplate",
            "铁甲胸甲",
            2,
            2,
            2.8,
            "common",
            "",
            1,
            1.0,
            0.19
        );
        InventoryItem broken = InventoryItem.createFull(
            2L,
            "armor_iron_chestplate",
            "铁甲胸甲",
            2,
            2,
            2.8,
            "common",
            "",
            1,
            1.0,
            0.0
        );

        assertEquals(0x60, GridSlotComponent.armorLowDurabilityFlashAlpha(low, 0L));
        assertEquals(0x20, GridSlotComponent.armorLowDurabilityFlashAlpha(low, 8L));
        assertEquals(0, GridSlotComponent.armorLowDurabilityFlashAlpha(broken, 0L));
    }
}
