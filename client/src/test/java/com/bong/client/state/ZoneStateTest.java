package com.bong.client.state;

import org.junit.jupiter.api.Test;

import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
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
        assertFalse(state.noCadence());
        assertEquals(0L, state.changedAtMillis());
    }

    @Test
    void createClampsDisplayValuesAndFallsBackLabel() {
        ZoneState state = ZoneState.create(" blood_valley ", "   ", 1.75, 8, -5L);

        assertEquals("blood_valley", state.zoneId());
        assertEquals("blood_valley", state.zoneLabel());
        assertEquals(1.0, state.spiritQiNormalized());
        assertEquals(5, state.dangerLevel());
        assertEquals(0L, state.changedAtMillis());
    }

    @Test
    void blankZoneIdTransitionsToEmptyState() {
        ZoneState state = ZoneState.create("   ", "Qingyun Peak", -0.2, -3, 99L);

        assertTrue(state.isEmpty());
        assertEquals(0.0, state.spiritQiNormalized());
        assertEquals(0, state.dangerLevel());
    }

    @Test
    void createMarksNoCadenceFromActiveEvents() {
        ZoneState state = ZoneState.create(
            "south_ash_dead_zone",
            "南荒余烬",
            0.0,
            5,
            Set.of("no_cadence"),
            12L
        );

        assertTrue(state.noCadence());
    }
}
