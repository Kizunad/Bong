package com.bong.client.combat.store;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class WoundsStoreTest {
    @AfterEach void tearDown() { WoundsStore.resetForTests(); }

    @Test void emptyByDefault() {
        assertTrue(WoundsStore.snapshot().isEmpty());
        assertFalse(WoundsStore.hasBleedingAny());
        assertEquals(0f, WoundsStore.maxInfection());
    }

    @Test void replacePopulatesSnapshot() {
        WoundsStore.replace(List.of(
            new WoundsStore.Wound("chest", "cut", 0.6f,
                WoundsStore.HealingState.BLEEDING, 0.3f, false, 1L),
            new WoundsStore.Wound("left_hand", "bone_fracture", 0.4f,
                WoundsStore.HealingState.STANCHED, 0f, false, 1L)
        ));
        assertEquals(2, WoundsStore.snapshot().size());
        assertTrue(WoundsStore.hasBleedingAny());
        assertEquals(0.3f, WoundsStore.maxInfection(), 1e-5);
    }

    @Test void healingStateFromWireFallsBackToBleeding() {
        assertEquals(WoundsStore.HealingState.BLEEDING, WoundsStore.HealingState.fromWire(null));
        assertEquals(WoundsStore.HealingState.STANCHED, WoundsStore.HealingState.fromWire("stanched"));
        assertEquals(WoundsStore.HealingState.HEALING, WoundsStore.HealingState.fromWire("healing"));
        assertEquals(WoundsStore.HealingState.SCARRED, WoundsStore.HealingState.fromWire("SCARRED"));
        assertEquals(WoundsStore.HealingState.BLEEDING, WoundsStore.HealingState.fromWire("unknown"));
    }

    @Test void replaceWithEmptyClears() {
        WoundsStore.replace(List.of(
            new WoundsStore.Wound("chest", "cut", 0.6f,
                WoundsStore.HealingState.BLEEDING, 0.3f, false, 1L)
        ));
        WoundsStore.replace(List.of());
        assertTrue(WoundsStore.snapshot().isEmpty());
    }
}
