package com.bong.client.network;

import com.bong.client.hud.CoffinStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class CoffinStateHandlerTest {
    @AfterEach
    void resetStore() {
        CoffinStateStore.resetForTests();
    }

    @Test
    void acceptsCoffinStatePayload() {
        String json = """
            {"type":"coffin_state","v":1,"in_coffin":true,"lifespan_rate_multiplier":0.9}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isHandled());
        assertTrue(CoffinStateStore.snapshot().inCoffin());
        assertEquals(0.9, CoffinStateStore.snapshot().lifespanRateMultiplier(), 1e-9);
    }

    @Test
    void rejectsInvalidMultiplier() {
        String json = """
            {"type":"coffin_state","v":1,"in_coffin":true,"lifespan_rate_multiplier":0.0}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isNoOp());
        assertFalse(CoffinStateStore.snapshot().inCoffin());
    }
}
