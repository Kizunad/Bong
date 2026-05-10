package com.bong.client.network;

import com.bong.client.tsy.ExtractStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class ExtractServerDataHandlerTest {
    @AfterEach
    void tearDown() {
        ExtractStateStore.resetForTests();
    }

    @Test
    void portalStateUpdatesStore() {
        ServerDataDispatch dispatch = new ExtractServerDataHandler().handle(parseEnvelope(
            "{\"v\":1,\"type\":\"rift_portal_state\",\"entity_id\":42,\"kind\":\"main_rift\",\"direction\":\"exit\",\"family_id\":\"tsy_lingxu_01\",\"world_pos\":[1,2,3],\"trigger_radius\":1.5,\"current_extract_ticks\":160}"
        ));

        assertTrue(dispatch.handled());
        assertEquals(1, ExtractStateStore.snapshot().portals().size());
        assertEquals(42L, ExtractStateStore.snapshot().portals().get(0).entityId());
        assertEquals(1.5, ExtractStateStore.snapshot().portals().get(0).triggerRadius());
    }

    @Test
    void portalRemovedEvictsStore() {
        ExtractServerDataHandler handler = new ExtractServerDataHandler();
        handler.handle(parseEnvelope(
            "{\"v\":1,\"type\":\"rift_portal_state\",\"entity_id\":42,\"kind\":\"main_rift\",\"direction\":\"exit\",\"family_id\":\"tsy_lingxu_01\",\"world_pos\":[1,2,3],\"trigger_radius\":1.5,\"current_extract_ticks\":160}"
        ));

        ServerDataDispatch dispatch = handler.handle(parseEnvelope(
            "{\"v\":1,\"type\":\"rift_portal_removed\",\"entity_id\":42}"
        ));

        assertTrue(dispatch.handled());
        assertEquals(0, ExtractStateStore.snapshot().portals().size());
    }

    @Test
    void extractProgressLifecycleUpdatesStore() {
        ExtractServerDataHandler handler = new ExtractServerDataHandler();
        handler.handle(parseEnvelope(
            "{\"v\":1,\"type\":\"extract_started\",\"player_id\":\"offline:Kiz\",\"portal_entity_id\":42,\"portal_kind\":\"main_rift\",\"required_ticks\":160,\"at_tick\":100}"
        ));
        handler.handle(parseEnvelope(
            "{\"v\":1,\"type\":\"extract_progress\",\"player_id\":\"offline:Kiz\",\"portal_entity_id\":42,\"elapsed_ticks\":20,\"required_ticks\":160}"
        ));

        assertTrue(ExtractStateStore.snapshot().extracting());
        assertEquals(20, ExtractStateStore.snapshot().elapsedTicks());

        handler.handle(parseEnvelope(
            "{\"v\":1,\"type\":\"extract_aborted\",\"player_id\":\"offline:Kiz\",\"reason\":\"out_of_range\"}"
        ));
        assertTrue(!ExtractStateStore.snapshot().extracting());
        assertTrue(ExtractStateStore.snapshot().message().contains("无法撤离"));
        assertTrue(ExtractStateStore.snapshot().message().contains("距离过远"));
    }

    @Test
    void portalOccupiedAbortUsesDedicatedRaceOutCopy() {
        new ExtractServerDataHandler().handle(parseEnvelope(
            "{\"v\":1,\"type\":\"extract_aborted\",\"player_id\":\"offline:Kiz\",\"reason\":\"portal_occupied\"}"
        ));

        assertTrue(ExtractStateStore.snapshot().message().contains("无法撤离"));
        assertTrue(ExtractStateStore.snapshot().message().contains("裂口被占，换下一个"));
    }

    @Test
    void collapsePayloadStartsCountdown() {
        new ExtractServerDataHandler().handle(parseEnvelope(
            "{\"v\":1,\"type\":\"tsy_collapse_started_ipc\",\"family_id\":\"tsy_lingxu_01\",\"at_tick\":100,\"remaining_ticks\":600,\"collapse_tear_entity_ids\":[1,2,3]}"
        ));

        assertTrue(ExtractStateStore.snapshot().collapseActive(System.currentTimeMillis()));
        assertEquals("tsy_lingxu_01", ExtractStateStore.snapshot().collapsingFamilyId());
    }

    @Test
    void completedPayloadTriggersWhiteFlash() {
        long now = System.currentTimeMillis();
        new ExtractServerDataHandler().handle(parseEnvelope(
            "{\"v\":1,\"type\":\"extract_completed\",\"player_id\":\"offline:Kiz\",\"portal_kind\":\"main_rift\",\"family_id\":\"tsy_lingxu_01\",\"exit_world_pos\":[8,65,9],\"at_tick\":200}"
        ));

        assertTrue(ExtractStateStore.snapshot().screenFlashActive(now));
        assertEquals(0xCCFFFFFF, ExtractStateStore.snapshot().screenFlashColor());
    }

    @Test
    void collapseCountdownReachingZeroTriggersWhiteFlash() {
        ExtractStateStore.markCollapseStarted("tsy_lingxu_01", 1, 1000L);

        ExtractStateStore.tick(1100L);

        assertTrue(ExtractStateStore.snapshot().screenFlashActive(1100L));
        assertEquals(0xCCFFFFFF, ExtractStateStore.snapshot().screenFlashColor());
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
