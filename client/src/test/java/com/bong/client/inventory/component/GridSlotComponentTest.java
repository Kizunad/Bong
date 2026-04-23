package com.bong.client.inventory.component;

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
}
