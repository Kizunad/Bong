package com.bong.client.combat.store;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class DerivedAttrsStoreTest {
    @AfterEach void tearDown() { DerivedAttrsStore.resetForTests(); }

    @Test void noneByDefault() {
        DerivedAttrsStore.State s = DerivedAttrsStore.snapshot();
        assertFalse(s.flying());
        assertFalse(s.phasing());
        assertFalse(s.tribulationLocked());
    }

    @Test void replaceRoundTrips() {
        DerivedAttrsStore.replace(new DerivedAttrsStore.State(
            true, 0.42f, 1000L, false, 0L, true, "warn", 0.85f, 2, true
        ));
        DerivedAttrsStore.State s = DerivedAttrsStore.snapshot();
        assertTrue(s.flying());
        assertEquals(0.42f, s.flyingQiRemaining(), 1e-5);
        assertTrue(s.tribulationLocked());
        assertEquals(2, s.vortexFakeSkinLayers());
    }

    @Test void nullReplacedByNone() {
        DerivedAttrsStore.replace(null);
        assertSame(DerivedAttrsStore.State.NONE, DerivedAttrsStore.snapshot());
    }
}
