package com.bong.client.inventory;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class RarityBorderRendererTest {
    @Test
    void mapsAllInventoryRaritiesToPlanColors() {
        assertEquals(0x808080, RarityBorderRenderer.colorRgb("common"));
        assertEquals(0x22CC22, RarityBorderRenderer.colorRgb("uncommon"));
        assertEquals(0x2288FF, RarityBorderRenderer.colorRgb("rare"));
        assertEquals(0xAA44FF, RarityBorderRenderer.colorRgb("epic"));
        assertEquals(0xFFAA00, RarityBorderRenderer.colorRgb("legendary"));
        assertEquals(0xFF4444, RarityBorderRenderer.colorRgb("ancient"));
    }

    @Test
    void ancientBorderAlphaBreathesBetweenHalfAndFullOpacity() {
        int low = RarityBorderRenderer.ancientPulseAlpha(30.0f);
        int high = RarityBorderRenderer.ancientPulseAlpha(10.0f);

        assertTrue(low >= 0x80 && low <= 0xFF);
        assertTrue(high >= 0x80 && high <= 0xFF);
        assertTrue(high > low, "half-period pulse should visibly change alpha");
    }

    @Test
    void ancientInvertFlashOnlyOccupiesEarlyPartOfThreeSecondCycle() {
        assertEquals(0x66, RarityBorderRenderer.ancientInvertFlashAlpha(0.0f));
        assertTrue(RarityBorderRenderer.ancientInvertFlashAlpha(2.0f) > 0);
        assertEquals(0, RarityBorderRenderer.ancientInvertFlashAlpha(5.0f));
        assertEquals(0x66, RarityBorderRenderer.ancientInvertFlashAlpha(60.0f));
    }
}
