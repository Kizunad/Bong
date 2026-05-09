package com.bong.client.network;

import com.bong.client.combat.store.FullPowerStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.*;

class FullPowerStateHandlerTest {
    private final FullPowerStateHandler handler = new FullPowerStateHandler();

    @AfterEach void tearDown() {
        FullPowerStateStore.resetForTests();
    }

    @Test void chargingPayloadUpdatesStore() {
        ServerDataDispatch dispatch = handler.handle(parse("""
            {"v":1,"type":"full_power_charging_state","caster_uuid":"offline:Azure","active":true,
             "qi_committed":40.0,"target_qi":80.0,"started_tick":1200}
            """), 1_000L);

        FullPowerStateStore.ChargingState state = FullPowerStateStore.charging();
        assertTrue(dispatch.handled());
        assertTrue(state.active());
        assertEquals("offline:Azure", state.casterUuid());
        assertEquals(0.5, state.progress(), 1e-6);
        assertEquals(1_000L, state.updatedAtMs());
    }

    @Test void releasePayloadRecordsEventAndClearsCharging() {
        FullPowerStateStore.updateCharging(new FullPowerStateStore.ChargingState(
            true, "offline:Azure", 20.0, 80.0, 100L, 900L
        ));

        ServerDataDispatch dispatch = handler.handle(parse("""
            {"v":1,"type":"full_power_release","caster_uuid":"offline:Azure","target_uuid":"npc:mantis",
             "qi_released":80.0,"tick":1300}
            """), 1_500L);

        FullPowerStateStore.ReleaseEvent event = FullPowerStateStore.lastRelease();
        assertTrue(dispatch.handled());
        assertFalse(FullPowerStateStore.charging().active());
        assertEquals("offline:Azure", event.casterUuid());
        assertEquals("npc:mantis", event.targetUuid());
        assertEquals(80.0, event.qiReleased(), 1e-6);
        assertEquals(1_300L, event.tick());
        assertEquals(1_500L, event.receivedAtMs());
    }

    @Test void exhaustedPayloadUpdatesAndClearsStore() {
        ServerDataDispatch activeDispatch = handler.handle(parse("""
            {"v":1,"type":"full_power_exhausted_state","caster_uuid":"offline:Azure","active":true,
             "started_tick":50,"recovery_at_tick":250}
            """), 5_000L);

        FullPowerStateStore.ExhaustedState state = FullPowerStateStore.exhausted();
        assertTrue(activeDispatch.handled());
        assertTrue(state.active());
        assertEquals("offline:Azure", state.casterUuid());
        assertEquals(200L, state.remainingTicks(5_000L));

        ServerDataDispatch clearDispatch = handler.handle(parse("""
            {"v":1,"type":"full_power_exhausted_state","caster_uuid":"offline:Azure","active":false,
             "started_tick":50,"recovery_at_tick":250}
            """), 6_000L);

        assertTrue(clearDispatch.handled());
        assertFalse(FullPowerStateStore.exhausted().active());
    }

    private static ServerDataEnvelope parse(String json) {
        ServerPayloadParseResult result = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(result.isSuccess(), result.errorMessage());
        return result.envelope();
    }
}
