package com.bong.client.combat.store;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class DeathStateStoreTest {
    @AfterEach void tearDown() { DeathStateStore.resetForTests(); }

    @Test void hiddenByDefault() {
        assertFalse(DeathStateStore.snapshot().visible());
    }

    @Test void replaceAndHide() {
        DeathStateStore.replace(new DeathStateStore.State(
            true, "pk", 0.5f, List.of("a", "b"), 10_000L, true, false
        ));
        assertTrue(DeathStateStore.snapshot().visible());
        DeathStateStore.hide();
        assertFalse(DeathStateStore.snapshot().visible());
    }

    @Test void remainingMsClampsToZero() {
        DeathStateStore.State s = new DeathStateStore.State(
            true, "pk", 0.5f, List.of(), 1000L, true, false
        );
        assertEquals(0L, s.remainingMs(9999L));
        assertEquals(500L, s.remainingMs(500L));
    }
}
