package com.bong.client.tsy;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;

public class TsyContainerStateStoreTest {
    @AfterEach
    void tearDown() {
        TsyContainerStateStore.resetForTests();
    }

    @Test
    void nearestInteractableSkipsDepletedOccupiedAndOutOfRange() {
        TsyContainerStateStore.upsert(new TsyContainerView(1, "dry_corpse", "f", 1, 0, 0, null, true, null));
        TsyContainerStateStore.upsert(new TsyContainerView(2, "dry_corpse", "f", 2, 0, 0, null, false, "other"));
        TsyContainerStateStore.upsert(new TsyContainerView(3, "dry_corpse", "f", 10, 0, 0, null, false, null));
        TsyContainerStateStore.upsert(new TsyContainerView(4, "storage_pouch", "f", 3, 0, 0, null, false, null));

        assertEquals(4L, TsyContainerStateStore.nearestInteractable(0, 0, 0, 5.0).entityId());
        assertNull(TsyContainerStateStore.nearestInteractable(0, 0, 0, 2.5));
    }

    @Test
    void tieUsesStableEntityId() {
        TsyContainerStateStore.upsert(new TsyContainerView(9, "dry_corpse", "f", 1, 0, 0, null, false, null));
        TsyContainerStateStore.upsert(new TsyContainerView(8, "dry_corpse", "f", -1, 0, 0, null, false, null));

        assertEquals(8L, TsyContainerStateStore.nearestInteractable(0, 0, 0, 5.0).entityId());
    }
}
