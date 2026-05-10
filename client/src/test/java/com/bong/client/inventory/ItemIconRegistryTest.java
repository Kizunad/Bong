package com.bong.client.inventory;

import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ItemIconRegistryTest {
    @Test
    void buildsCanonicalItemTexturePaths() {
        assertEquals(
            "bong-client:textures/gui/items/bone_coin_40.png",
            ItemIconRegistry.itemTexturePath("bone_coin_40")
        );
    }

    @Test
    void exposesBotanyThumbnailsThroughCentralRegistry() {
        assertEquals(
            "bong-client:textures/gui/botany/ci_she_hao.png",
            ItemIconRegistry.plantIconPath("ci_she_hao")
        );
        assertNull(ItemIconRegistry.plantIconPath("unknown_plant"));
    }

    @Test
    void scrollLikeIdsUseScrollFallback() {
        assertTrue(ItemIconRegistry.isScrollTextureCandidate("blueprint_scroll_qing_feng"));
        assertEquals(
            new Identifier(ItemIconRegistry.FALLBACK_SCROLL_PATH),
            ItemIconRegistry.fallbackTextureIdForItemId("blueprint_scroll_qing_feng")
        );
    }
}
