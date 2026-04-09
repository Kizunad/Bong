package com.bong.client;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class PlayerStateHandlerTest {
    @BeforeEach
    void setUp() {
        PlayerStateState.clear();
    }

    @AfterEach
    void tearDown() {
        PlayerStateState.clear();
    }

    @Test
    public void typedPlayerStatePayloadRoutesIntoLocalState() {
        BongServerPayload.PlayerStatePayload payload = new BongServerPayload.PlayerStatePayload(
                1,
                new BongServerPayload.PlayerState("qi_refining_3", 78.0d, 100.0d, -0.2d, 0.35d, "blood_valley")
        );

        assertTrue(BongServerPayloadRouter.route(null, payload));

        PlayerStateState.PlayerStateSnapshot snapshot = PlayerStateState.getCurrentPlayerState();
        assertNotNull(snapshot);
        assertEquals("qi_refining_3", snapshot.realmKey());
        assertEquals(78.0d, snapshot.spiritQi());
        assertEquals(100.0d, snapshot.spiritQiMax());
        assertEquals(-0.2d, snapshot.karma());
        assertEquals(0.35d, snapshot.compositePower());
        assertEquals("blood_valley", snapshot.zoneKey());
    }

    @Test
    public void handlerClampsInvalidValuesIntoSafeLocalState() {
        BongServerPayload.PlayerStatePayload payload = new BongServerPayload.PlayerStatePayload(
                1,
                new BongServerPayload.PlayerState(
                        "foundation_2",
                        120.0d,
                        -4.0d,
                        9.0d,
                        7.0d,
                        ""
                )
        );

        PlayerStateHandler.handle(payload, 5_000L);

        PlayerStateState.PlayerStateSnapshot snapshot = PlayerStateState.getCurrentPlayerState();
        assertNotNull(snapshot);
        assertEquals("foundation_2", snapshot.realmKey());
        assertEquals(1.0d, snapshot.spiritQi());
        assertEquals(1.0d, snapshot.spiritQiMax());
        assertEquals(1.0d, snapshot.karma());
        assertEquals(1.0d, snapshot.compositePower());
        assertEquals("unknown_zone", snapshot.zoneKey());
        assertEquals(5_000L, snapshot.updatedAtMs());
    }
}
