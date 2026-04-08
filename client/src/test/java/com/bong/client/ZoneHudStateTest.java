package com.bong.client;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class ZoneHudStateTest {
    @AfterEach
    void tearDown() {
        ZoneHudState.clear();
    }

    @Test
    void updateStoresZoneSnapshotAndEntryBannerWindow() {
        ZoneHudState.update("spawn", 0.9, 0, List.of(), 1_000L);

        ZoneHudState.ZoneSnapshot snapshot = ZoneHudState.peek();
        assertNotNull(snapshot);
        assertEquals("spawn", snapshot.zone());
        assertEquals(0.9, snapshot.spiritQi());
        assertEquals(0, snapshot.dangerLevel());
        assertEquals(3_000L, snapshot.entryBannerExpiresAtMs());
        assertTrue(ZoneHudState.shouldShowEntryBanner(2_999L));
        assertFalse(ZoneHudState.shouldShowEntryBanner(3_000L));
    }

    @Test
    void invalidInputsAreIgnoredWithoutCorruptingState() {
        ZoneHudState.update("spawn", 0.9, 0, List.of(), 1_000L);
        ZoneHudState.update("", 0.8, 1, List.of("beast_tide"), 2_000L);

        ZoneHudState.ZoneSnapshot snapshot = ZoneHudState.peek();
        assertNotNull(snapshot);
        assertEquals("spawn", snapshot.zone());
        assertEquals(0.9, snapshot.spiritQi());
        assertEquals(0, snapshot.dangerLevel());
    }

    @Test
    void clearRemovesSnapshot() {
        ZoneHudState.update("spawn", 0.9, 0, List.of(), 1_000L);
        ZoneHudState.clear();
        assertNull(ZoneHudState.peek());
    }
}
