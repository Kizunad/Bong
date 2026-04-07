package com.bong.client;

import com.bong.client.network.handlers.ZoneInfoHandler;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;

public class ZoneInfoHandlerTest {
    private final ZoneInfoHandler handler = new ZoneInfoHandler();

    @AfterEach
    void tearDown() {
        ZoneHudState.clear();
    }

    @Test
    void validZoneInfoPayloadUpdatesHudState() {
        String json = "{" +
            "\"v\":1," +
            "\"type\":\"zone_info\"," +
            "\"zone\":\"blood_valley\"," +
            "\"spirit_qi\":0.42," +
            "\"danger_level\":4," +
            "\"active_events\":[\"beast_tide\"]" +
            "}";

        handler.handle(null, "zone_info", json);

        ZoneHudState.ZoneSnapshot snapshot = ZoneHudState.peek();
        assertNotNull(snapshot);
        assertEquals("blood_valley", snapshot.zone());
        assertEquals(0.42, snapshot.spiritQi());
        assertEquals(4, snapshot.dangerLevel());
        assertEquals(1, snapshot.activeEvents().size());
        assertEquals("beast_tide", snapshot.activeEvents().get(0));
    }

    @Test
    void malformedZoneInfoPayloadIsIgnoredSafely() {
        String validJson = "{" +
            "\"v\":1," +
            "\"type\":\"zone_info\"," +
            "\"zone\":\"spawn\"," +
            "\"spirit_qi\":0.9," +
            "\"danger_level\":0" +
            "}";
        handler.handle(null, "zone_info", validJson);

        String malformedJson = "{" +
            "\"v\":1," +
            "\"type\":\"zone_info\"," +
            "\"zone\":\"blood_valley\"," +
            "\"danger_level\":4" +
            "}";
        handler.handle(null, "zone_info", malformedJson);

        ZoneHudState.ZoneSnapshot snapshot = ZoneHudState.peek();
        assertNotNull(snapshot);
        assertEquals("spawn", snapshot.zone());
        assertEquals(0.9, snapshot.spiritQi());
        assertEquals(0, snapshot.dangerLevel());
    }
}
