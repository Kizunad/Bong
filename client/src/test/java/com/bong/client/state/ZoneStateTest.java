package com.bong.client.state;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class ZoneStateTest {
    @Test
    void emptyFactoryReturnsNoActiveZone() {
        ZoneState state = ZoneState.empty();

        assertTrue(state.isEmpty());
        assertEquals("", state.zoneId());
        assertEquals("", state.zoneLabel());
        assertEquals(0.0, state.spiritQiNormalized());
        assertEquals(0, state.dangerLevel());
        assertEquals("normal", state.status());
        assertEquals(0L, state.changedAtMillis());
    }

    @Test
    void createClampsDisplayValuesAndFallsBackLabel() {
        ZoneState state = ZoneState.create(" blood_valley ", "   ", 1.75, 8, -5L);

        assertEquals("blood_valley", state.zoneId());
        assertEquals("blood_valley", state.zoneLabel());
        assertEquals(1.0, state.spiritQiNormalized());
        assertEquals(5, state.dangerLevel());
        assertEquals("normal", state.status());
        assertEquals(0L, state.changedAtMillis());
    }

    @Test
    void createPreservesCollapsedStatusAndDefaultsUnknownStatus() {
        ZoneState collapsed = ZoneState.create("blood_valley", "Blood Valley", 0.1, 5, " collapsed ", 10L);
        ZoneState unknown = ZoneState.create("jade_valley", "Jade Valley", 0.5, 1, "fading", 11L);

        assertEquals("collapsed", collapsed.status());
        assertTrue(collapsed.collapsed());
        assertEquals("normal", unknown.status());
    }

    @Test
    void blankZoneIdTransitionsToEmptyState() {
        ZoneState state = ZoneState.create("   ", "Qingyun Peak", -0.2, -3, 99L);

        assertTrue(state.isEmpty());
        assertEquals(0.0, state.spiritQiNormalized());
        assertEquals(0, state.dangerLevel());
    }
}
