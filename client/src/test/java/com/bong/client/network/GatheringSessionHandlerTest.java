package com.bong.client.network;

import com.bong.client.gathering.GatheringSessionStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class GatheringSessionHandlerTest {
    @AfterEach
    void tearDown() {
        GatheringSessionStore.resetForTests();
    }

    @Test
    void routeUpdatesGatheringSessionStore() {
        String json = """
            {"v":1,"type":"gathering_session","session_id":"p1","progress_ticks":10,"total_ticks":40,"target_name":"凝脉草","target_type":"herb","quality_hint":"fine_likely","tool_used":"hoe_iron","interrupted":false,"completed":false}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter
            .createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError());
        assertTrue(result.isHandled());
        assertEquals("p1", GatheringSessionStore.snapshot().sessionId());
        assertEquals(0.25, GatheringSessionStore.snapshot().progressRatio(), 0.0001);
        assertEquals("优良", GatheringSessionStore.snapshot().qualityLabel());
    }

    @Test
    void routeClampsInvalidTickFieldsWithoutDroppingSession() {
        String json = """
            {"v":1,"type":"gathering_session","session_id":"p1","progress_ticks":-3,"total_ticks":0,"target_name":"铜矿","target_type":"ore","quality_hint":"perfect","interrupted":false,"completed":false}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter
            .createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError());
        assertTrue(result.isHandled());
        assertEquals("p1", GatheringSessionStore.snapshot().sessionId());
        assertEquals(0.0, GatheringSessionStore.snapshot().progressRatio(), 0.0001);
        assertEquals("极品", GatheringSessionStore.snapshot().qualityLabel());
    }

    @Test
    void routeFallsBackWhenTickNumberOverflowsLong() {
        String json = """
            {"v":1,"type":"gathering_session","session_id":"p1","progress_ticks":9223372036854775808,"total_ticks":40,"target_name":"灵木","target_type":"wood","quality_hint":"normal","interrupted":false,"completed":false}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter
            .createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError());
        assertTrue(result.isHandled());
        assertEquals(0.0, GatheringSessionStore.snapshot().progressRatio(), 0.0001);
    }
}
