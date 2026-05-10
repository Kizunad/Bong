package com.bong.client.inventory.render;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class DroppedLootRarityVisualsTest {
    @Test
    void rareAndAboveDropsGetAuraParticles() {
        assertFalse(DroppedLootRarityVisuals.hasAuraParticles("uncommon"));
        assertTrue(DroppedLootRarityVisuals.hasAuraParticles("rare"));
        assertEquals(2, DroppedLootRarityVisuals.auraParticleCount("rare"));
        assertEquals(4, DroppedLootRarityVisuals.auraParticleCount("legendary"));
    }

    @Test
    void legendaryAndAncientDropsGetBeamAndOnlyAncientHums() {
        assertEquals(0.0, DroppedLootRarityVisuals.beamHeight("epic"), 0.0001);
        assertEquals(1.0, DroppedLootRarityVisuals.beamHeight("legendary"), 0.0001);
        assertEquals(1.35, DroppedLootRarityVisuals.beamHeight("ancient"), 0.0001);
        assertFalse(DroppedLootRarityVisuals.shouldHum("legendary"));
        assertTrue(DroppedLootRarityVisuals.shouldHum("ancient"));
        assertTrue(DroppedLootRarityVisuals.isAncient(" Ancient "));
    }
}
