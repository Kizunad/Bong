package com.bong.client.combat.handler;

import com.bong.client.combat.store.DerivedAttrsStore;
import com.bong.client.combat.store.FalseSkinHudStateStore;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerPayloadParseResult;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class FalseSkinStateHandlerTest {
    @AfterEach
    void tearDown() {
        DerivedAttrsStore.resetForTests();
        FalseSkinHudStateStore.resetForTests();
    }

    @Test
    void appliesLegacyFalseSkinStateToDerivedAttrsAndHudStore() {
        ServerDataDispatch dispatch = new FalseSkinStateHandler().handle(parseEnvelope("""
            {"v":1,"type":"false_skin_state",
             "target_id":"offline:Azure","kind":"rotten_wood_armor","layers_remaining":2,
             "contam_capacity_per_layer":100.0,"absorbed_contam":35.0,"equipped_at_tick":7}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        assertEquals(2, DerivedAttrsStore.snapshot().tuikeLayers());
        FalseSkinHudStateStore.State state = FalseSkinHudStateStore.snapshot();
        assertEquals("offline:Azure", state.targetId());
        assertEquals(2, state.layersRemaining());
        assertEquals(0.35f, state.contamRatio(), 1e-6);
        assertEquals(2, state.layers().size());
    }

    @Test
    void acceptsRichLayerDetailsWhenPayloadCarriesThem() {
        new FalseSkinStateHandler().handle(parseEnvelope("""
            {"v":1,"type":"false_skin_state",
             "target_id":"offline:Azure","kind":"rotten_wood_armor","layers_remaining":3,
             "contam_capacity_per_layer":100.0,"absorbed_contam":80.0,"equipped_at_tick":9,
             "layers":[
               {"tier":"fan","spirit_quality":0.7,"damage_capacity":20.0,"contam_load":0.0,"permanent_taint_load":0.0},
               {"tier":"mid","spirit_quality":1.4,"damage_capacity":150.0,"contam_load":12.0,"permanent_taint_load":0.0},
               {"tier":"ancient","spirit_quality":3.0,"damage_capacity":1000.0,"contam_load":80.0,"permanent_taint_load":0.5}
             ]}
            """));

        FalseSkinHudStateStore.State state = FalseSkinHudStateStore.snapshot();
        assertEquals(3, state.layersRemaining());
        assertEquals("ancient", state.layers().get(2).tier());
        assertEquals(3.0f, state.layers().get(2).spiritQuality(), 1e-6);
    }

    @Test
    void clampsNegativeNumericPayloadFieldsBeforeStoringHudState() {
        new FalseSkinStateHandler().handle(parseEnvelope("""
            {"v":1,"type":"false_skin_state",
             "target_id":"offline:Azure","kind":"rotten_wood_armor","layers_remaining":1,
             "contam_capacity_per_layer":-100.0,"absorbed_contam":-35.0,"equipped_at_tick":-7,
             "layers":[
               {"tier":"fan","spirit_quality":-0.7,"damage_capacity":-20.0,
                "contam_load":-1.0,"permanent_taint_load":-0.5}
             ]}
            """));

        FalseSkinHudStateStore.State state = FalseSkinHudStateStore.snapshot();
        assertEquals(1, DerivedAttrsStore.snapshot().tuikeLayers());
        assertEquals(1, state.layersRemaining());
        assertEquals(0.0f, state.contamCapacityPerLayer(), 1e-6);
        assertEquals(0.0f, state.absorbedContam(), 1e-6);
        assertEquals(0L, state.equippedAtTick());
        assertEquals(0.0f, state.layers().get(0).spiritQuality(), 1e-6);
        assertEquals(0.0f, state.layers().get(0).damageCapacity(), 1e-6);
        assertEquals(0.0f, state.layers().get(0).contamLoad(), 1e-6);
        assertEquals(0.0f, state.layers().get(0).permanentTaintLoad(), 1e-6);
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            json, json.getBytes(StandardCharsets.UTF_8).length
        );
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
