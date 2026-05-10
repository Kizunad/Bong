package com.bong.client.network;

import com.bong.client.hud.PoisonTraitHudStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class PoisonTraitServerDataHandlerTest {
    @AfterEach
    void clearStore() {
        PoisonTraitHudStateStore.clear();
    }

    @Test
    void statePayloadUpdatesHudStore() {
        String json = """
            {"v":1,"type":"poison_trait_state","player_entity_id":7,"poison_toxicity":42.0,"digestion_current":35.0,"digestion_capacity":120.0,"toxicity_tier_unlocked":true}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isNoOp());
        PoisonTraitHudStateStore.State state = PoisonTraitHudStateStore.snapshot();
        assertTrue(state.active());
        assertEquals(42.0f, state.toxicity(), 0.001f);
        assertEquals(35.0f, state.digestionCurrent(), 0.001f);
        assertEquals(120.0f, state.digestionCapacity(), 0.001f);
    }

    @Test
    void overdosePayloadRaisesLifespanWarning() {
        PoisonTraitServerDataHandler handler = new PoisonTraitServerDataHandler(() -> 10_000L);
        ServerDataEnvelope envelope = ServerDataEnvelope.parse(
            """
                {"v":1,"type":"poison_overdose_event","player_entity_id":7,"severity":"moderate","overflow":30.0,"lifespan_penalty_years":1.0,"micro_tear_probability":0.1,"at_tick":120}
                """,
            200
        ).envelope();

        ServerDataDispatch dispatch = handler.handle(envelope);

        assertTrue(dispatch.handled());
        PoisonTraitHudStateStore.State state = PoisonTraitHudStateStore.snapshot();
        assertTrue(state.active());
        assertEquals(11_500L, state.lifespanWarningUntilMillis());
        assertEquals(1.0f, state.lifespanYearsLost(), 0.001f);
    }

    @Test
    void malformedDoseBecomesNoOp() {
        String json = """
            {"v":1,"type":"poison_dose_event","player_entity_id":7,"dose_amount":5.0,"side_effect_tag":"qi_focus_drift_2h","poison_level_after":150.0,"digestion_after":20.0,"at_tick":100}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isNoOp());
    }
}
