package com.bong.client.block;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class BlockPickerCatalogTest {

    private static BlockPickerCatalog.Entry vanillaBlock(String path) {
        return new BlockPickerCatalog.Entry("minecraft", path, true);
    }

    @Test
    void keepsOnlyVanillaBlocksWithBlockItemsSortedAndDeduped() {
        List<String> result = BlockPickerCatalog.filterPlaceableShortNames(List.of(
            vanillaBlock("stone"),
            vanillaBlock("dirt"),
            vanillaBlock("stone"), // duplicate must collapse
            vanillaBlock("stone_bricks")
        ));
        assertEquals(
            List.of("dirt", "stone", "stone_bricks"),
            result,
            "filterPlaceableShortNames must dedupe and lexicographically sort vanilla block short names, actual=" + result
        );
    }

    @Test
    void dropsNonVanillaNamespaceBlocks() {
        List<String> result = BlockPickerCatalog.filterPlaceableShortNames(List.of(
            new BlockPickerCatalog.Entry("bong", "zhenfa_node", true),
            vanillaBlock("stone")
        ));
        assertEquals(
            List.of("stone"),
            result,
            "bong-namespaced blocks must be excluded — dev block panel only exposes vanilla blocks, actual=" + result
        );
        assertFalse(result.contains("zhenfa_node"),
            "bong block must never leak into the vanilla block picker list");
    }

    @Test
    void dropsBlocksWithoutBlockItem() {
        List<String> result = BlockPickerCatalog.filterPlaceableShortNames(List.of(
            new BlockPickerCatalog.Entry("minecraft", "fire", false),  // no obtainable item
            new BlockPickerCatalog.Entry("minecraft", "water", false), // no obtainable item
            vanillaBlock("stone")
        ));
        assertEquals(
            List.of("stone"),
            result,
            "blocks without a BlockItem cannot be given as items and must be dropped, actual=" + result
        );
    }

    @Test
    void dropsBlankAndNullEntries() {
        List<String> result = BlockPickerCatalog.filterPlaceableShortNames(java.util.Arrays.asList(
            null,
            new BlockPickerCatalog.Entry("minecraft", "", true),
            new BlockPickerCatalog.Entry("minecraft", "  ", true),
            vanillaBlock("dirt")
        ));
        assertEquals(
            List.of("dirt"),
            result,
            "null entries and blank paths must be dropped, actual=" + result
        );
    }

    @Test
    void emptyAndNullInputsYieldEmptyList() {
        assertTrue(BlockPickerCatalog.filterPlaceableShortNames(List.of()).isEmpty(),
            "empty entries must yield empty list");
        assertTrue(BlockPickerCatalog.filterPlaceableShortNames(null).isEmpty(),
            "null entries must yield empty list (no NPE)");
    }
}
