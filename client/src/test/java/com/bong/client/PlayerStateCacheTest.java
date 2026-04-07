package com.bong.client;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;

public class PlayerStateCacheTest {
    @AfterEach
    void tearDown() {
        PlayerStateCache.clear();
    }

    @Test
    void invalidSnapshotUpdateDoesNotCorruptExistingState() {
        PlayerStateCache.update(
            "qi_refining_2",
            34.0,
            -0.1,
            0.22,
            new PlayerStateCache.PowerBreakdown(0.2, 0.1, 0.3, 0.1, 0.05),
            "spawn"
        );

        PlayerStateCache.update(
            "qi_refining_3",
            50.0,
            0.2,
            1.4,
            new PlayerStateCache.PowerBreakdown(0.2, 0.1, 0.3, 0.1, 0.05),
            "qingyun_peak"
        );

        PlayerStateCache.PlayerStateSnapshot snapshot = PlayerStateCache.peek();
        assertNotNull(snapshot);
        assertEquals("qi_refining_2", snapshot.realm());
        assertEquals(34.0, snapshot.spiritQi());
        assertEquals(-0.1, snapshot.karma());
        assertEquals(0.22, snapshot.compositePower());
        assertEquals("spawn", snapshot.zone());
    }
}
