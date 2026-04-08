package com.bong.client;

import com.bong.client.network.handlers.PlayerStateHandler;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;

public class PlayerStateHandlerTest {
    private final PlayerStateHandler handler = new PlayerStateHandler();

    @AfterEach
    void tearDown() {
        PlayerStateCache.clear();
    }

    @Test
    void validPlayerStatePayloadUpdatesCache() {
        String json = "{" +
            "\"v\":1," +
            "\"type\":\"player_state\"," +
            "\"realm\":\"qi_refining_3\"," +
            "\"spirit_qi\":78," +
            "\"karma\":0.2," +
            "\"composite_power\":0.35," +
            "\"breakdown\":{" +
            "\"combat\":0.2," +
            "\"wealth\":0.4," +
            "\"social\":0.65," +
            "\"karma\":0.2," +
            "\"territory\":0.1}," +
            "\"zone\":\"qingyun_peak\"" +
            "}";

        handler.handle(null, "player_state", json);

        PlayerStateCache.PlayerStateSnapshot snapshot = PlayerStateCache.peek();
        assertNotNull(snapshot);
        assertEquals("qi_refining_3", snapshot.realm());
        assertEquals(78.0, snapshot.spiritQi());
        assertEquals(0.2, snapshot.karma());
        assertEquals(0.35, snapshot.compositePower());
        assertEquals(0.2, snapshot.breakdown().combat());
        assertEquals(0.4, snapshot.breakdown().wealth());
        assertEquals(0.65, snapshot.breakdown().social());
        assertEquals(0.2, snapshot.breakdown().karma());
        assertEquals(0.1, snapshot.breakdown().territory());
        assertEquals("qingyun_peak", snapshot.zone());
    }

    @Test
    void malformedPlayerStatePayloadIsIgnoredSafely() {
        String validJson = "{" +
            "\"v\":1," +
            "\"type\":\"player_state\"," +
            "\"realm\":\"mortal\"," +
            "\"spirit_qi\":10," +
            "\"karma\":0.0," +
            "\"composite_power\":0.05," +
            "\"breakdown\":{" +
            "\"combat\":0.05," +
            "\"wealth\":0.02," +
            "\"social\":0.1," +
            "\"karma\":0.0," +
            "\"territory\":0.01}," +
            "\"zone\":\"spawn\"" +
            "}";
        handler.handle(null, "player_state", validJson);

        String malformedJson = "{" +
            "\"v\":1," +
            "\"type\":\"player_state\"," +
            "\"realm\":\"qi_refining_3\"," +
            "\"spirit_qi\":81," +
            "\"karma\":0.2," +
            "\"composite_power\":0.35," +
            "\"breakdown\":{" +
            "\"combat\":1.4," +
            "\"wealth\":0.4," +
            "\"social\":0.65," +
            "\"karma\":0.2," +
            "\"territory\":0.1}," +
            "\"zone\":\"qingyun_peak\"" +
            "}";
        handler.handle(null, "player_state", malformedJson);

        PlayerStateCache.PlayerStateSnapshot snapshot = PlayerStateCache.peek();
        assertNotNull(snapshot);
        assertEquals("mortal", snapshot.realm());
        assertEquals(10.0, snapshot.spiritQi());
        assertEquals(0.0, snapshot.karma());
        assertEquals(0.05, snapshot.compositePower());
        assertEquals("spawn", snapshot.zone());
    }
}
