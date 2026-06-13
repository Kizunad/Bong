package com.bong.client.block;

import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class BlockPickerFilterTest {

    private static final List<String> CATALOG = List.of(
        "stone", "stone_bricks", "smooth_stone", "dirt", "sand"
    );

    @Test
    void blankQueryReturnsAllPreservingOrder() {
        assertEquals(
            CATALOG,
            BlockPickerFilter.filter(CATALOG, ""),
            "empty query must return all entries in input order"
        );
        assertEquals(
            CATALOG,
            BlockPickerFilter.filter(CATALOG, "   "),
            "whitespace-only query must be treated as empty"
        );
        assertEquals(
            CATALOG,
            BlockPickerFilter.filter(CATALOG, null),
            "null query must be treated as empty (no NPE)"
        );
    }

    @Test
    void substringMatchIsCaseInsensitive() {
        assertEquals(
            List.of("stone", "stone_bricks", "smooth_stone"),
            BlockPickerFilter.filter(CATALOG, "STONE"),
            "uppercase query must match lowercase ids (case-insensitive substring), actual="
                + BlockPickerFilter.filter(CATALOG, "STONE")
        );
    }

    @Test
    void substringMatchesAnywhereNotJustPrefix() {
        assertEquals(
            List.of("stone_bricks"),
            BlockPickerFilter.filter(CATALOG, "brick"),
            "query must match anywhere in the id, not only as a prefix"
        );
    }

    @Test
    void noMatchYieldsEmptyList() {
        assertTrue(
            BlockPickerFilter.filter(CATALOG, "diamond").isEmpty(),
            "non-matching query must yield empty list"
        );
    }

    @Test
    void emptyOrNullCatalogYieldsEmptyList() {
        assertTrue(BlockPickerFilter.filter(List.of(), "stone").isEmpty());
        assertTrue(BlockPickerFilter.filter(null, "stone").isEmpty());
    }

    @Test
    void resultIsTruncatedToMaxVisible() {
        List<String> large = new ArrayList<>();
        for (int i = 0; i < BlockPickerFilter.MAX_VISIBLE + 50; i++) {
            large.add("block_" + i);
        }
        List<String> result = BlockPickerFilter.filter(large, "");
        assertEquals(
            BlockPickerFilter.MAX_VISIBLE,
            result.size(),
            "filter must cap results at MAX_VISIBLE to keep owo render cheap, actual=" + result.size()
        );
    }
}
