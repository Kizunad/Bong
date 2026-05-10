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

    @Test
    void droppedLootSyncPreservesAncientChargesMetadata() {
        String payload = """
            {
              "v": 1,
              "type": "dropped_loot_sync",
              "drops": [
                {
                  "instance_id": 9001,
                  "source_container_id": "main_pack",
                  "source_row": 0,
                  "source_col": 0,
                  "world_pos": [1.0, 64.0, 2.0],
                  "item": {
                    "instance_id": 9001,
                    "item_id": "ancient_relic",
                    "display_name": "上古遗物",
                    "grid_width": 1,
                    "grid_height": 1,
                    "weight": 0.5,
                    "rarity": "ancient",
                    "description": "",
                    "stack_count": 1,
                    "spirit_quality": 0.0,
                    "durability": 1.0,
                    "charges": 3
                  }
                }
              ]
            }
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route(payload, 0);

        assertTrue(result.isHandled(), result.logMessage());
        assertEquals(3, DroppedItemStore.get(9001L).item().charges());
    }

    @Test
    void malformedDroppedLootPositionIsRejectedWithoutThrowing() {
        String payload = """
            {
              "v": 1,
              "type": "dropped_loot_sync",
              "drops": [
                {
                  "instance_id": 9002,
                  "source_container_id": "main_pack",
                  "source_row": 0,
                  "source_col": 0,
                  "world_pos": [1.0, 64.0],
                  "item": {
                    "instance_id": 9002,
                    "item_id": "rare_relic",
                    "display_name": "稀有遗物",
                    "grid_width": 1,
                    "grid_height": 1,
                    "weight": 0.5,
                    "rarity": "rare",
                    "description": "",
                    "stack_count": 1,
                    "spirit_quality": 1.0,
                    "durability": 1.0
                  }
                }
              ]
            }
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route(payload, 0);

        assertFalse(result.isHandled(), result.logMessage());
        assertTrue(DroppedItemStore.snapshot().isEmpty());
    }

    @Test
    void droppedLootSyncRejectsInvalidChargesField() {
        String payload = """
            {
              "v": 1,
              "type": "dropped_loot_sync",
              "drops": [
                {
                  "instance_id": 9100,
                  "source_container_id": "main_pack",
                  "source_row": 0,
                  "source_col": 0,
                  "world_pos": [1.0, 64.0, 2.0],
                  "item": {
                    "instance_id": 9100,
                    "item_id": "ancient_relic",
                    "display_name": "上古遗物",
                    "grid_width": 1,
                    "grid_height": 1,
                    "weight": 0.5,
                    "rarity": "ancient",
                    "description": "",
                    "stack_count": 1,
                    "spirit_quality": 1.0,
                    "durability": 1.0,
                    "charges": "bad"
                  }
                }
              ]
            }
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route(payload, 0);

        assertFalse(result.isHandled(), result.logMessage());
        assertTrue(DroppedItemStore.snapshot().isEmpty());
    }
}
