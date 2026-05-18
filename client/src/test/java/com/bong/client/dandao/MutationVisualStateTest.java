package com.bong.client.dandao;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-dandao-path-v1 P3 -- Tests for MutationVisualState client state store.
 */
class MutationVisualStateTest {
    @AfterEach
    void cleanup() {
        MutationVisualState.reset();
    }

    @Test
    void initialStateIsEmpty() {
        assertEquals(0, MutationVisualState.stage(), "Default stage should be 0");
        assertEquals(0.0, MutationVisualState.cumulativeToxin(), 0.001, "Default toxin should be 0");
        assertEquals(0.0, MutationVisualState.meridianPenalty(), 0.001, "Default penalty should be 0");
        assertTrue(MutationVisualState.activeSlots().isEmpty(), "Default slots should be empty");
    }

    @Test
    void updateSetsAllFields() {
        List<MutationVisualState.MutationSlotEntry> slots = List.of(
            new MutationVisualState.MutationSlotEntry("GoldenIris", "Head", 1),
            new MutationVisualState.MutationSlotEntry("ForearmScales", "Forearm", 2)
        );

        MutationVisualState.update(2, 150.0, 0.08, slots);

        assertEquals(2, MutationVisualState.stage());
        assertEquals(150.0, MutationVisualState.cumulativeToxin(), 0.001);
        assertEquals(0.08, MutationVisualState.meridianPenalty(), 0.001);
        assertEquals(2, MutationVisualState.activeSlots().size());
        assertEquals("GoldenIris", MutationVisualState.activeSlots().get(0).kind());
        assertEquals("Head", MutationVisualState.activeSlots().get(0).bodySlot());
        assertEquals(1, MutationVisualState.activeSlots().get(0).level());
    }

    @Test
    void updateReplacesExistingState() {
        MutationVisualState.update(1, 50.0, 0.03, List.of(
            new MutationVisualState.MutationSlotEntry("GoldenIris", "Head", 1)
        ));
        assertEquals(1, MutationVisualState.stage());

        MutationVisualState.update(3, 300.0, 0.15, List.of(
            new MutationVisualState.MutationSlotEntry("Horns", "Head", 2),
            new MutationVisualState.MutationSlotEntry("BackCarapace", "Back", 1)
        ));

        assertEquals(3, MutationVisualState.stage());
        assertEquals(300.0, MutationVisualState.cumulativeToxin(), 0.001);
        assertEquals(2, MutationVisualState.activeSlots().size());
    }

    @Test
    void resetClearsAllState() {
        MutationVisualState.update(4, 600.0, 0.20, List.of(
            new MutationVisualState.MutationSlotEntry("ExtraArms", "Torso", 3)
        ));
        MutationVisualState.reset();

        assertEquals(0, MutationVisualState.stage());
        assertEquals(0.0, MutationVisualState.cumulativeToxin(), 0.001);
        assertTrue(MutationVisualState.activeSlots().isEmpty());
    }

    @Test
    void activeSlotsReturnsDefensiveCopy() {
        MutationVisualState.update(1, 50.0, 0.03, List.of(
            new MutationVisualState.MutationSlotEntry("GoldenIris", "Head", 1)
        ));

        List<MutationVisualState.MutationSlotEntry> copy = MutationVisualState.activeSlots();
        assertThrows(UnsupportedOperationException.class, () -> copy.add(
            new MutationVisualState.MutationSlotEntry("Horns", "Head", 1)
        ), "activeSlots() should return unmodifiable list");
    }

    @Test
    void slotEntryRecordAccessors() {
        var entry = new MutationVisualState.MutationSlotEntry("BeastFace", "Head", 3);
        assertEquals("BeastFace", entry.kind());
        assertEquals("Head", entry.bodySlot());
        assertEquals(3, entry.level());
    }
}
