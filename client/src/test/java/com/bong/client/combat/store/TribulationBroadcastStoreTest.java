package com.bong.client.combat.store;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class TribulationBroadcastStoreTest {
    @AfterEach void tearDown() { TribulationBroadcastStore.resetForTests(); }

    @Test void clearsWhenInactive() {
        TribulationBroadcastStore.replace(new TribulationBroadcastStore.State(
            true, "甲", "warn", 0, 0, 9_999L, false, 0
        ));
        TribulationBroadcastStore.clear();
        assertFalse(TribulationBroadcastStore.snapshot().active());
    }

    @Test void expiredDetected() {
        TribulationBroadcastStore.State s = new TribulationBroadcastStore.State(
            true, "甲", "warn", 0, 0, 1_000L, false, 0
        );
        assertTrue(s.expired(2_000L));
        assertFalse(s.expired(500L));
    }

    @Test void neverExpiresWhenZero() {
        TribulationBroadcastStore.State s = new TribulationBroadcastStore.State(
            true, "", "", 0, 0, 0L, false, 0
        );
        assertFalse(s.expired(Long.MAX_VALUE));
    }
}
