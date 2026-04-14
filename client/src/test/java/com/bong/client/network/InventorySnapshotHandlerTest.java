package com.bong.client.network;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Optional;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class InventorySnapshotHandlerTest {
    private static final Path SHARED_SCHEMA_SAMPLES_DIR = Path.of("..", "agent", "packages", "schema", "samples");

    @BeforeEach
    void setUp() {
        InventoryStateStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        InventoryStateStore.resetForTests();
    }

    @Test
    void sharedFixtureRoutesIntoAuthoritativeInventoryStore() throws IOException {
        String json = loadSharedFixture("server-data.inventory-snapshot.sample.json");
        ServerDataRouter router = ServerDataRouter.createDefault();

        // Shared schema sample is intentionally human-readable and may exceed transport budget,
        // so this test validates router+handler semantics directly against parsed fixture shape.
        ServerDataRouter.RouteResult result = router.route(json, 0);

        assertFalse(result.isParseError(), result.logMessage());
        assertTrue(result.isHandled(), result.logMessage());
        assertEquals("inventory_snapshot", result.envelope().type());
        assertTrue(InventoryStateStore.isAuthoritativeLoaded());
        assertEquals(12L, InventoryStateStore.revision());

        InventoryModel snapshot = InventoryStateStore.snapshot();
        assertNotNull(snapshot);
        assertEquals(3, snapshot.containers().size());
        assertEquals(InventoryModel.PRIMARY_CONTAINER_ID, snapshot.containers().get(0).id());
        assertEquals(InventoryModel.SMALL_POUCH_CONTAINER_ID, snapshot.containers().get(1).id());
        assertEquals(InventoryModel.FRONT_SATCHEL_CONTAINER_ID, snapshot.containers().get(2).id());

        InventoryModel.GridEntry sentinel = snapshot.gridItems().stream()
            .filter(entry -> "starter_talisman".equals(entry.item().itemId()))
            .findFirst()
            .orElseThrow();
        assertEquals(InventoryModel.PRIMARY_CONTAINER_ID, sentinel.containerId());
        assertEquals(0, sentinel.row());
        assertEquals(0, sentinel.col());
        assertEquals(1001L, sentinel.item().instanceId());
        assertEquals(1, sentinel.item().stackCount());
        assertEquals(0.76, sentinel.item().spiritQuality(), 0.0001);
        assertEquals(0.93, sentinel.item().durability(), 0.0001);

        InventoryItem mainHand = snapshot.equipped().get(EquipSlotType.MAIN_HAND);
        assertNotNull(mainHand);
        assertEquals("training_blade", mainHand.itemId());
        assertEquals(1003L, mainHand.instanceId());

        InventoryItem hotbar0 = snapshot.hotbar().get(0);
        assertNotNull(hotbar0);
        assertEquals("healing_draught", hotbar0.itemId());
        assertEquals(2, hotbar0.stackCount());

        assertEquals(57L, snapshot.boneCoins());
        assertEquals(3.5, snapshot.currentWeight(), 0.0001);
        assertEquals(50.0, snapshot.maxWeight(), 0.0001);
        assertEquals("qi_refining_1", snapshot.realm());
        assertEquals(24.0, snapshot.qiCurrent(), 0.0001);
        assertEquals(100.0, snapshot.qiMax(), 0.0001);
        assertEquals(0.18, snapshot.bodyLevel(), 0.0001);
    }

    @Test
    void nestedSnapshotWrapperIsSafelyIgnoredBecauseHandlerExpectsDirectRootShape() {
        String wrapped = """
            {
              "v": 1,
              "type": "inventory_snapshot",
              "snapshot": {
                "revision": 12,
                "containers": [],
                "placed_items": [],
                "equipped": {},
                "hotbar": [],
                "bone_coins": 1,
                "weight": {"current": 0, "max": 50},
                "realm": "qi_refining_1",
                "qi_current": 1,
                "qi_max": 1,
                "body_level": 0.1
              }
            }
            """;

        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            wrapped,
            wrapped.getBytes(StandardCharsets.UTF_8).length
        );
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());

        ServerDataDispatch dispatch = new InventorySnapshotHandler().handle(parseResult.envelope());
        assertFalse(dispatch.handled());
        assertTrue(dispatch.logMessage().contains("missing or invalid required root field"));
        assertFalse(InventoryStateStore.isAuthoritativeLoaded());
        assertEquals(-1L, InventoryStateStore.revision());
    }

    private static String loadSharedFixture(String fileName) throws IOException {
        Path fixturePath = SHARED_SCHEMA_SAMPLES_DIR.resolve(fileName);
        return Files.readString(fixturePath, StandardCharsets.UTF_8);
    }
}
