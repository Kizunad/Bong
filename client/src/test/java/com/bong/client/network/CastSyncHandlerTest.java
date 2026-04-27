package com.bong.client.network;

import com.bong.client.combat.CastOutcome;
import com.bong.client.combat.CastState;
import com.bong.client.combat.CastStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class CastSyncHandlerTest {
    @BeforeEach
    void setUp() { CastStateStore.resetForTests(); }
    @AfterEach
    void tearDown() { CastStateStore.resetForTests(); }

    @Test
    void appliesCastingPhase() {
        ServerDataDispatch dispatch = new CastSyncHandler().handle(parseEnvelope("""
            {"v":1,"type":"cast_sync","phase":"casting","slot":3,
             "duration_ms":1500,"started_at_ms":1700000000000,"outcome":"none"}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        CastState state = CastStateStore.snapshot();
        assertEquals(CastState.Phase.CASTING, state.phase());
        assertEquals(3, state.slot());
        assertEquals(1500, state.durationMs());
    }

    @Test
    void appliesInterruptPhaseWithOutcome() {
        ServerDataDispatch dispatch = new CastSyncHandler().handle(parseEnvelope("""
            {"v":1,"type":"cast_sync","phase":"interrupt","slot":0,
             "duration_ms":1500,"started_at_ms":1700000000000,"outcome":"interrupt_contam"}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        CastState state = CastStateStore.snapshot();
        assertEquals(CastState.Phase.INTERRUPT, state.phase());
        assertEquals(CastOutcome.INTERRUPT_CONTAM, state.outcome());
    }

    @Test
    void rejectsUnknownPhase() {
        ServerDataDispatch dispatch = new CastSyncHandler().handle(parseEnvelope("""
            {"v":1,"type":"cast_sync","phase":"sleeping","slot":0,
             "duration_ms":1500,"started_at_ms":1700000000000,"outcome":"none"}
            """));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.logMessage().contains("unknown phase"));
    }

    @Test
    void rejectsOutOfRangeSlot() {
        ServerDataDispatch dispatch = new CastSyncHandler().handle(parseEnvelope("""
            {"v":1,"type":"cast_sync","phase":"casting","slot":42,
             "duration_ms":1500,"started_at_ms":1700000000000,"outcome":"none"}
            """));

        assertFalse(dispatch.handled());
    }

    @Test
    void completedSkillBarCastDoesNotRelabelNextQuickSlotCast() {
        CastStateStore.beginSkillBarCast(2, 500, 1000L);
        CastStateStore.complete(1500L);

        ServerDataDispatch dispatch = new CastSyncHandler().handle(parseEnvelope("""
            {"v":1,"type":"cast_sync","phase":"casting","slot":2,
             "duration_ms":1500,"started_at_ms":1700000000000,"outcome":"none"}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        assertEquals(CastState.Source.QUICK_SLOT, CastStateStore.snapshot().source());
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
