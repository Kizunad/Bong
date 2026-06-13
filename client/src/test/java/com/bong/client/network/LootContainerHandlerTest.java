package com.bong.client.network;

import com.bong.client.hud.LootContainerStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class LootContainerHandlerTest {
    @AfterEach
    void resetStore() {
        LootContainerStateStore.resetForTests();
    }

    @Test
    void openSetsStateStoreSession() {
        String json = """
            {"type":"loot_container_open","v":1,"session_id":42,"source_kind":{"kind":"supply_coffin","grade":"common"},"rows":3,"cols":4,"placed_items":[],"timeout_wall_secs":1716872400}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isHandled(), "loot_container_open should be handled");
        assertTrue(LootContainerStateStore.isOpen(), "state store should be open after open payload");

        LootContainerStateStore.OpenSession session =
            assertInstanceOf(LootContainerStateStore.OpenSession.class, LootContainerStateStore.current());
        assertEquals(42, session.sessionId());
        assertEquals("supply_coffin", session.sourceKind());
        assertEquals("common", session.grade());
        assertEquals(3, session.rows());
        assertEquals(4, session.cols());
        assertEquals(1716872400L, session.timeoutWallSecs());
    }

    @Test
    void openParsesRustExternalTaggedStorageCrateSourceKind() {
        String json = """
            {"type":"loot_container_open","v":1,"session_id":43,"source_kind":{"storage_crate":{"is_herb":true}},"rows":4,"cols":4,"placed_items":[],"timeout_wall_secs":0}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isHandled(), "storage crate loot_container_open should be handled");
        LootContainerStateStore.OpenSession session =
            assertInstanceOf(LootContainerStateStore.OpenSession.class, LootContainerStateStore.current());
        assertEquals("storage_crate", session.sourceKind());
        assertEquals("herb", session.grade(), "is_herb=true should be preserved for UI labels");
        assertEquals(4, session.rows());
        assertEquals(4, session.cols());
    }

    @Test
    void openTreatsMalformedStorageCrateHerbFlagAsTradeCrate() {
        String json = """
            {"type":"loot_container_open","v":1,"session_id":43,"source_kind":{"storage_crate":{"is_herb":"yes"}},"rows":4,"cols":4,"placed_items":[],"timeout_wall_secs":0}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isHandled(), "malformed is_herb should not crash the handler");
        LootContainerStateStore.OpenSession session =
            assertInstanceOf(LootContainerStateStore.OpenSession.class, LootContainerStateStore.current());
        assertEquals("storage_crate", session.sourceKind());
        assertEquals("trade", session.grade(), "non-boolean is_herb should fall back to trade crate");
    }

    @Test
    void openParsesRustExternalTaggedDeadDropSourceKind() {
        String json = """
            {"type":"loot_container_open","v":1,"session_id":44,"source_kind":"dead_drop","rows":3,"cols":3,"placed_items":[],"timeout_wall_secs":0}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isHandled(), "dead drop loot_container_open should be handled");
        LootContainerStateStore.OpenSession session =
            assertInstanceOf(LootContainerStateStore.OpenSession.class, LootContainerStateStore.current());
        assertEquals("dead_drop", session.sourceKind());
        assertEquals(3, session.rows());
        assertEquals(3, session.cols());
    }

    @Test
    void closeClearsStateStoreForMatchingSession() {
        LootContainerStateStore.open(new LootContainerStateStore.OpenSession(
            42, "supply_coffin", "common", 3, 4, 1716872400L, java.util.List.of()
        ));
        assertTrue(LootContainerStateStore.isOpen());

        String json = """
            {"type":"loot_container_close","v":1,"session_id":42,"reason":"timeout"}
            """;
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isHandled(), "loot_container_close should be handled");
        assertFalse(LootContainerStateStore.isOpen(), "state store should be closed after close payload");
        assertNull(LootContainerStateStore.current(), "current should be null after close");
    }

    @Test
    void closeIgnoresMismatchedSessionId() {
        LootContainerStateStore.open(new LootContainerStateStore.OpenSession(
            42, "supply_coffin", "rare", 4, 5, 1716872500L, java.util.List.of()
        ));

        String json = """
            {"type":"loot_container_close","v":1,"session_id":99,"reason":"distance"}
            """;
        ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(LootContainerStateStore.isOpen(),
            "state store should remain open when close targets a different session_id");
    }

    @Test
    void updateHandledAndStateReflectsEmptyItems() {
        LootContainerStateStore.open(new LootContainerStateStore.OpenSession(
            42, "supply_coffin", "common", 3, 4, 1716872400L, java.util.List.of()
        ));

        String json = """
            {"type":"loot_container_update","v":1,"session_id":42,"placed_items":[]}
            """;
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isHandled(), "loot_container_update should be handled");
        assertTrue(LootContainerStateStore.isOpen(),
            "store should remain open after update");
        LootContainerStateStore.OpenSession session =
            assertInstanceOf(LootContainerStateStore.OpenSession.class, LootContainerStateStore.current());
        assertTrue(session.placedItems().isEmpty(),
            "placedItems should be empty after update with empty placed_items");
        assertEquals(42, session.sessionId(),
            "sessionId should still match after update");
    }

    @Test
    void openRejectsMissingSessionId() {
        String json = """
            {"type":"loot_container_open","v":1,"rows":3,"cols":4,"placed_items":[],"timeout_wall_secs":0}
            """;
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isNoOp(), "missing session_id should be a no-op");
        assertFalse(LootContainerStateStore.isOpen());
    }

    @Test
    void listenerNotifiedOnOpenAndClose() {
        var received = new java.util.concurrent.atomic.AtomicReference<LootContainerStateStore.Session>();
        LootContainerStateStore.Listener listener = received::set;
        LootContainerStateStore.addListener(listener);

        try {
            LootContainerStateStore.open(new LootContainerStateStore.OpenSession(
                7, "supply_coffin", "precious", 5, 6, 1716872600L, java.util.List.of()
            ));
            assertInstanceOf(LootContainerStateStore.OpenSession.class, received.get(),
                "listener should receive OpenSession on open");

            LootContainerStateStore.close(7, "player_closed");
            assertInstanceOf(LootContainerStateStore.Closed.class, received.get(),
                "listener should receive Closed on close");
            assertEquals("player_closed",
                ((LootContainerStateStore.Closed) received.get()).reason());
        } finally {
            LootContainerStateStore.removeListener(listener);
        }
    }

    @Test
    void allCloseReasonsAccepted() {
        String[] reasons = {
            "timeout",
            "distance",
            "player_closed",
            "coffin_destroyed",
            "container_destroyed"
        };
        var received = new java.util.concurrent.atomic.AtomicReference<LootContainerStateStore.Session>();
        LootContainerStateStore.Listener listener = received::set;
        LootContainerStateStore.addListener(listener);
        try {
            for (String reason : reasons) {
                LootContainerStateStore.open(new LootContainerStateStore.OpenSession(
                    1, "supply_coffin", "common", 3, 4, 0L, java.util.List.of()
                ));
                String json = """
                    {"type":"loot_container_close","v":1,"session_id":1,"reason":"%s"}
                    """.formatted(reason);
                ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
                    .route(json, json.getBytes(StandardCharsets.UTF_8).length);
                assertTrue(result.isHandled(),
                    "close with reason '" + reason + "' should be handled");
                assertFalse(LootContainerStateStore.isOpen(),
                    "store should be closed after reason '" + reason + "'");
                LootContainerStateStore.Closed closed =
                    assertInstanceOf(LootContainerStateStore.Closed.class, received.get(),
                        "listener should receive Closed after reason '" + reason + "'");
                assertEquals(reason, closed.reason(),
                    "closed payload reason should be preserved for '" + reason + "'");
            }
        } finally {
            LootContainerStateStore.removeListener(listener);
        }
    }

    @Test
    void closeRejectsUnknownReasonWithoutClosingOrNotifying() {
        LootContainerStateStore.open(new LootContainerStateStore.OpenSession(
            1, "supply_coffin", "common", 3, 4, 0L, java.util.List.of()
        ));
        var received = new java.util.concurrent.atomic.AtomicReference<LootContainerStateStore.Session>();
        LootContainerStateStore.Listener listener = received::set;
        LootContainerStateStore.addListener(listener);

        try {
            String json = """
                {"type":"loot_container_close","v":1,"session_id":1,"reason":"invalid_reason"}
                """;
            ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
                .route(json, json.getBytes(StandardCharsets.UTF_8).length);

            assertTrue(result.isNoOp(),
                "close with an unknown reason should be rejected as a schema violation");
            assertTrue(LootContainerStateStore.isOpen(),
                "store should remain open after an unknown close reason");
            assertNull(received.get(),
                "listener should not receive Closed for an unknown close reason");
        } finally {
            LootContainerStateStore.removeListener(listener);
        }
    }
}
