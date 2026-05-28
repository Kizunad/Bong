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
        assertEquals(3, session.rows());
        assertEquals(4, session.cols());
        assertEquals(1716872400L, session.timeoutWallSecs());
    }

    @Test
    void closeClearsStateStoreForMatchingSession() {
        LootContainerStateStore.open(new LootContainerStateStore.OpenSession(
            42, "supply_coffin", "common", 3, 4, 1716872400L
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
            42, "supply_coffin", "rare", 4, 5, 1716872500L
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
    void updateHandledWithoutError() {
        String json = """
            {"type":"loot_container_update","v":1,"session_id":42,"placed_items":[]}
            """;
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
            .route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isHandled(), "loot_container_update should be handled");
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
                7, "supply_coffin", "precious", 5, 6, 1716872600L
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
        String[] reasons = {"timeout", "distance", "player_closed", "coffin_destroyed"};
        for (String reason : reasons) {
            LootContainerStateStore.open(new LootContainerStateStore.OpenSession(
                1, "supply_coffin", "common", 3, 4, 0L
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
        }
    }
}
