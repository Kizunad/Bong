package com.bong.client.network;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class InventoryEventHandlerTest {
    @BeforeEach
    void setUp() {
        InventoryStateStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        InventoryStateStore.resetForTests();
    }

    @Test
    void eventBeforeAuthoritativeSnapshotIsIgnoredSafely() {
        ServerDataDispatch dispatch = new InventoryEventHandler().handle(parseEnvelope("""
            {"v":1,"type":"inventory_event","kind":"stack_changed","revision":13,"instance_id":1004,"stack_count":1}
            """));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.logMessage().contains("snapshot is not loaded"));
        assertFalse(InventoryStateStore.isAuthoritativeLoaded());
        assertEquals(-1L, InventoryStateStore.revision());
        assertTrue(InventoryStateStore.snapshot().isEmpty());
    }

    @Test
    void staleRevisionIsIgnoredSafelyWithoutMutatingStore() {
        InventoryModel baseline = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .gridItem(
                InventoryItem.createFull(
                    1001L,
                    "starter_talisman",
                    "启程护符",
                    1,
                    1,
                    0.2,
                    "uncommon",
                    "初入修途者配发的护身符。",
                    1,
                    0.76,
                    0.93
                ),
                InventoryModel.PRIMARY_CONTAINER_ID,
                0,
                0
            )
            .build();
        InventoryStateStore.applyAuthoritativeSnapshot(baseline, 12L);

        ServerDataDispatch dispatch = new InventoryEventHandler().handle(parseEnvelope("""
            {"v":1,"type":"inventory_event","kind":"stack_changed","revision":11,"instance_id":1004,"stack_count":1}
            """));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.logMessage().contains("stale"));
        assertEquals(12L, InventoryStateStore.revision());
        assertTrue(InventoryStateStore.isAuthoritativeLoaded());
        assertEquals(baseline, InventoryStateStore.snapshot());
    }

    @Test
    void unsupportedKindIsIgnoredSafelyWithoutMutatingStore() {
        InventoryModel baseline = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .gridItem(
                InventoryItem.createFull(
                    1001L,
                    "starter_talisman",
                    "启程护符",
                    1,
                    1,
                    0.2,
                    "uncommon",
                    "初入修途者配发的护身符。",
                    1,
                    0.76,
                    0.93
                ),
                InventoryModel.PRIMARY_CONTAINER_ID,
                0,
                0
            )
            .build();
        InventoryStateStore.applyAuthoritativeSnapshot(baseline, 12L);

        ServerDataDispatch dispatch = new InventoryEventHandler().handle(parseEnvelope("""
            {"v":1,"type":"inventory_event","kind":"teleported","revision":13,"instance_id":1001}
            """));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.logMessage().contains("unsupported"));
        assertEquals(12L, InventoryStateStore.revision());
        assertTrue(InventoryStateStore.isAuthoritativeLoaded());
        assertEquals(baseline, InventoryStateStore.snapshot());
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            json,
            json.getBytes(StandardCharsets.UTF_8).length
        );
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
