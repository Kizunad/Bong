package com.bong.client.block;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class BlockVanillaIconMapTest {
    @Test
    void createStackForMapsKnownBongBlocksToVanillaBlockItems() {
        assertEquals("minecraft:dirt", BlockVanillaIconMap.vanillaItemIdForTests("earth_crumb"));
        assertEquals("minecraft:coarse_dirt", BlockVanillaIconMap.vanillaItemIdForTests("hardened_soil"));
        assertEquals("minecraft:sand", BlockVanillaIconMap.vanillaItemIdForTests("barren_sand"));
        assertEquals("minecraft:gravel", BlockVanillaIconMap.vanillaItemIdForTests("weathered_stone"));
        assertEquals("minecraft:clay", BlockVanillaIconMap.vanillaItemIdForTests("raw_clay_lump"));
        assertEquals("minecraft:obsidian", BlockVanillaIconMap.vanillaItemIdForTests("obsidian_shard"));
    }

    @Test
    void recognizesKnownBlocksAndReturnsEmptyForUnknown() {
        assertTrue(BlockVanillaIconMap.isKnownBlockItem("earth_crumb"));
        assertFalse(BlockVanillaIconMap.isKnownBlockItem("missing_block"));
        assertTrue(BlockVanillaIconMap.createStackFor("missing_block").isEmpty());
    }
}
