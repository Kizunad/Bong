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

    @Test void keepsConcurrentBroadcastsAndClearsTargetOnly() {
        TribulationBroadcastStore.upsert(new TribulationBroadcastStore.State(
            true, "近处", "warn", 0, 0, 10_000L, true, 30
        ));
        TribulationBroadcastStore.upsert(new TribulationBroadcastStore.State(
            true, "远处", "warn", 400, 0, 10_000L, false, 400
        ));

        assertEquals(2, TribulationBroadcastStore.all().size());
        assertEquals("近处", TribulationBroadcastStore.snapshot(1_000L).actorName());

        TribulationBroadcastStore.clear(new TribulationBroadcastStore.State(
            false, "近处", "done", 0, 0, 0L, false, 0
        ));

        assertEquals(1, TribulationBroadcastStore.all().size());
        assertEquals("远处", TribulationBroadcastStore.snapshot(1_000L).actorName());
    }

    @Test void expiredEntriesAreNotSelectedAsPrimary() {
        TribulationBroadcastStore.upsert(new TribulationBroadcastStore.State(
            true, "旧劫", "striking", 0, 0, 500L, true, 10
        ));
        TribulationBroadcastStore.upsert(new TribulationBroadcastStore.State(
            true, "新劫", "warn", 400, 0, 10_000L, false, 400
        ));

        assertEquals("新劫", TribulationBroadcastStore.snapshot(1_000L).actorName());
    }
}
