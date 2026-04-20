package com.bong.client.network;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.state.DroppedItemStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class DroppedLootSyncHandlerTest {
    private static final Path SHARED_SCHEMA_SAMPLES_DIR = Path.of("..", "agent", "packages", "schema", "samples");

    @AfterEach
    void tearDown() {
        DroppedItemStore.resetForTests();
    }

    @Test
    void sharedFixtureReplacesDroppedItemStore() throws IOException {
        String json = Files.readString(SHARED_SCHEMA_SAMPLES_DIR.resolve("server-data.dropped-loot-sync.sample.json"));
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route(json, 0);

        assertFalse(result.isParseError(), result.logMessage());
        assertTrue(result.isHandled(), result.logMessage());
        assertEquals("dropped_loot_sync", result.envelope().type());
        assertEquals(1, DroppedItemStore.snapshot().size());
        DroppedItemStore.Entry entry = DroppedItemStore.get(1004L);
        assertEquals("main_pack", entry.sourceContainerId());
        assertEquals(8.5, entry.worldPosX());
        assertEquals("starter_talisman", entry.item().itemId());
    }

    @Test
    void emptyDroppedLootSyncClearsExistingStore() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
            1004L,
            "main_pack",
            0,
            0,
            8.5,
            66.0,
            8.5,
            InventoryItem.simple("starter_talisman", "启程护符")
        ));

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route(
            "{\"v\":1,\"type\":\"dropped_loot_sync\",\"drops\":[]}",
            0
        );

        assertFalse(result.isParseError(), result.logMessage());
        assertTrue(result.isHandled(), result.logMessage());
        assertTrue(DroppedItemStore.snapshot().isEmpty());
    }
}
