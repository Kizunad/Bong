package com.bong.client.inventory.state;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.lifecycle.SessionScopedStoreRegistry;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class InventoryStoreRegistryAdapterTest {

    @BeforeEach
    void setUp() {
        RemainsStore.resetForTests();
        DroppedItemStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        RemainsStore.resetForTests();
        DroppedItemStore.resetForTests();
    }

    @Test
    void productionRegistryClearsEntriesWithoutResettingMonotonicCounters() {
        RemainsStore.putOrReplace(new RemainsStore.Entry(
            "old-remains",
            1.0,
            64.0,
            1.0,
            "minecraft:overworld",
            "旧遗骸",
            3,
            12L
        ));
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1L,
            "main_pack",
            0,
            0,
            1.0,
            64.0,
            1.0,
            InventoryItem.simple("old-item", "旧物")
        ));
        assertEquals(1L, RemainsStore.insertionCounterForTests());
        assertEquals(1L, DroppedItemStore.insertionCounterForTests());

        SessionScopedStoreRegistry.clearAllOnDisconnect();

        assertTrue(RemainsStore.snapshot().isEmpty());
        assertTrue(DroppedItemStore.snapshot().isEmpty());
        assertEquals(
            1L,
            RemainsStore.insertionCounterForTests(),
            "production registry 必须绑定 clearOnDisconnect，不能误绑会归零 counter 的 test reset"
        );
        assertEquals(
            1L,
            DroppedItemStore.insertionCounterForTests(),
            "production registry 必须绑定 clearOnDisconnect，不能误绑会归零 counter 的 test reset"
        );

        RemainsStore.putOrReplace(new RemainsStore.Entry(
            "fresh-remains",
            2.0,
            64.0,
            2.0,
            "minecraft:overworld",
            "新遗骸",
            1,
            0L
        ));
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            2L,
            "main_pack",
            0,
            1,
            2.0,
            64.0,
            2.0,
            InventoryItem.simple("fresh-item", "新物")
        ));

        assertEquals(2L, RemainsStore.insertionCounterForTests());
        assertEquals(2L, DroppedItemStore.insertionCounterForTests());
    }
}
