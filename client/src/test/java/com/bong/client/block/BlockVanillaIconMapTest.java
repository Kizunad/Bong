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
        assertEquals("minecraft:crafting_table", BlockVanillaIconMap.vanillaItemIdForTests("workbench_item"));
        assertEquals("minecraft:torch", BlockVanillaIconMap.vanillaItemIdForTests("torch_item"));
        assertEquals("minecraft:lantern", BlockVanillaIconMap.vanillaItemIdForTests("lantern_item"));
        assertEquals("minecraft:iron_door", BlockVanillaIconMap.vanillaItemIdForTests("door_bolt"));
        assertEquals("minecraft:iron_bars", BlockVanillaIconMap.vanillaItemIdForTests("window_grate"));
    }

    @Test
    void recognizesKnownBlocksAndReturnsEmptyForUnknown() {
        assertTrue(BlockVanillaIconMap.isKnownBlockItem("earth_crumb"));
        assertTrue(BlockVanillaIconMap.isKnownBlockItem("workbench_item"));
        assertTrue(BlockVanillaIconMap.isKnownBlockItem("torch_item"));
        assertTrue(BlockVanillaIconMap.isKnownBlockItem("window_grate"));
        assertFalse(BlockVanillaIconMap.isKnownBlockItem("missing_block"));
        assertTrue(BlockVanillaIconMap.createStackFor("missing_block").isEmpty());
    }
}
