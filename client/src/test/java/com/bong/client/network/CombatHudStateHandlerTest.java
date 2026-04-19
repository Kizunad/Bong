package com.bong.client.network;

import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.CombatHudStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class CombatHudStateHandlerTest {
    @BeforeEach
    void setUp() {
        CombatHudStateStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        CombatHudStateStore.resetForTests();
    }

    @Test
    void appliesValidPayloadToStore() {
        ServerDataDispatch dispatch = new CombatHudStateHandler().handle(parseEnvelope("""
            {"v":1,"type":"combat_hud_state",
             "hp_percent":1.0,"qi_percent":0.42,"stamina_percent":0.85,
             "derived":{"flying":true,"phasing":false,"tribulation_locked":false}}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        CombatHudState state = CombatHudStateStore.snapshot();
        assertEquals(0.42f, state.qiPercent(), 1e-6);
        assertEquals(0.85f, state.staminaPercent(), 1e-6);
        assertTrue(state.derived().flying());
        assertFalse(state.derived().phasing());
    }

    @Test
    void rejectsMissingDerivedField() {
        ServerDataDispatch dispatch = new CombatHudStateHandler().handle(parseEnvelope("""
            {"v":1,"type":"combat_hud_state",
             "hp_percent":1.0,"qi_percent":0.5,"stamina_percent":0.5}
            """));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.logMessage().contains("required fields"));
    }

    @Test
    void rejectsOutOfRangePercent() {
        ServerDataDispatch dispatch = new CombatHudStateHandler().handle(parseEnvelope("""
            {"v":1,"type":"combat_hud_state",
             "hp_percent":1.0,"qi_percent":1.5,"stamina_percent":0.5,
             "derived":{"flying":false,"phasing":false,"tribulation_locked":false}}
            """));

        assertFalse(dispatch.handled());
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            json, json.getBytes(StandardCharsets.UTF_8).length
        );
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
