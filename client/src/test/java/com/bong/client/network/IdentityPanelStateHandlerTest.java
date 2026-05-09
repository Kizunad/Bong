package com.bong.client.network;

import com.bong.client.identity.IdentityPanelState;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

final class IdentityPanelStateHandlerTest {
    @Test
    void parsesIdentityPanelStateIntoDispatch() {
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route("""
            {
              "v": 1,
              "type": "identity_panel_state",
              "active_identity_id": 1,
              "last_switch_tick": 200,
              "cooldown_remaining_ticks": 0,
              "identities": [
                {
                  "identity_id": 0,
                  "display_name": "旧名",
                  "reputation_score": -80,
                  "frozen": true,
                  "revealed_tag_kinds": ["dugu_revealed"]
                },
                {
                  "identity_id": 1,
                  "display_name": "新名",
                  "reputation_score": 0,
                  "frozen": false,
                  "revealed_tag_kinds": []
                }
              ]
            }
            """, 0);

        assertFalse(result.isParseError());
        assertTrue(result.isHandled());
        IdentityPanelState state = result.dispatch().identityPanelState().orElseThrow();
        assertEquals(1, state.activeIdentityId());
        assertEquals(2, state.identities().size());
        assertEquals("新名", state.activeEntry().orElseThrow().displayName());
        assertEquals("dugu_revealed", state.identities().get(0).revealedTagKinds().get(0));
    }

    @Test
    void malformedEntryBecomesNoOp() {
        String json = """
            {"v":1,"type":"identity_panel_state","active_identity_id":0,"last_switch_tick":0,"cooldown_remaining_ticks":0,
             "identities":[{"identity_id":0,"display_name":"kiz","reputation_score":0,"frozen":false}]}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError());
        assertTrue(result.isNoOp());
        assertTrue(result.logMessage().contains("malformed identity entry"));
    }
}
