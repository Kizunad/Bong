package com.bong.client;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class ZoneStateTest {
    @BeforeEach
    void setUp() {
        ZoneState.clear();
    }

    @AfterEach
    void tearDown() {
        ZoneState.clear();
    }

    @Test
    public void typedZonePayloadRoutesIntoZoneState() {
        BongServerPayload.ZoneInfoPayload payload = new BongServerPayload.ZoneInfoPayload(
                1,
                new BongServerPayload.ZoneInfo("blood_valley", 0.42d, 3, null)
        );

        assertTrue(BongServerPayloadRouter.route(null, payload));

        ZoneState.ZoneHudState zone = ZoneState.getCurrentZone();
        assertNotNull(zone);
        assertEquals("Blood Valley", zone.zoneLabel());
        assertEquals(0.42d, zone.spiritQi());
        assertEquals(3, zone.dangerLevel());
    }

    @Test
    public void zoneStateClampsValuesAndClipsLongLabels() {
        ZoneState.ZoneHudState snapshot = ZoneState.snapshotOf(
                new BongServerPayload.ZoneInfo("the_extremely_long_and_windy_blood_valley_of_echoes", 4.2d, 99, null),
                5_000L
        );

        assertEquals("The Extremely Long An...", snapshot.zoneLabel());
        assertEquals(1.0d, snapshot.spiritQi());
        assertEquals(5, snapshot.dangerLevel());
        assertEquals(5_000L, snapshot.changedAtMs());
    }

    @Test
    public void bigTitleAlphaFadesPredictably() {
        assertEquals(255, BongZoneHud.bigTitleAlpha(1_000L, 1_000L));
        assertEquals(255, BongZoneHud.bigTitleAlpha(2_500L, 1_000L));
        assertEquals(128, BongZoneHud.bigTitleAlpha(2_750L, 1_000L));
        assertEquals(0, BongZoneHud.bigTitleAlpha(3_000L, 1_000L));
    }

    @Test
    public void emptyZoneStateRemainsClear() {
        assertNull(ZoneState.getCurrentZone());
    }
}
