package com.bong.client.network;

import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class BreakthroughCinematicHandlerTest {
    @Test
    void handlesBreakthroughCinematicServerDataWithVisualAndToast() {
        String json = """
            {
              "v": 1,
              "type": "breakthrough_cinematic",
              "actor_id": "offline:Kiz",
              "phase": "apex",
              "phase_tick": 0,
              "phase_duration_ticks": 80,
              "realm_from": "Condense",
              "realm_to": "Solidify",
              "result": "success",
              "interrupted": false,
              "world_pos": [12.0, 64.0, -8.0],
              "visible_radius_blocks": 1024.0,
              "global": false,
              "distant_billboard": true,
              "particle_density": 2.2,
              "intensity": 0.78,
              "season_overlay": "adaptive",
              "style": "golden_core",
              "at_tick": 2400
            }
            """;

        ServerDataRouter.RouteResult result =
            ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError());
        assertTrue(result.isHandled());
        assertEquals("breakthrough_cinematic", result.envelope().type());
        assertTrue(result.dispatch().visualEffectState().isPresent());
        assertTrue(result.dispatch().alertToast().isPresent());
        assertTrue(result.logMessage().contains("bong:breakthrough_pillar"));
    }

    @Test
    void rejectsMalformedBreakthroughCinematicPayloadSafely() {
        String json = """
            {
              "v": 1,
              "type": "breakthrough_cinematic",
              "phase": "apex"
            }
            """;

        ServerDataRouter.RouteResult result =
            ServerDataRouter.createDefault().route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError());
        assertTrue(result.isNoOp());
    }
}
