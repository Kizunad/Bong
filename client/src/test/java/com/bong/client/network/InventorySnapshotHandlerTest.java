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

    @Test
    void rejectsPlacedItemWhoseFootprintOverflowsTargetContainerBounds() {
        String overflow = """
            {
              "v": 1,
              "type": "inventory_snapshot",
              "revision": 12,
              "containers": [
                {"id":"main_pack","name":"主背包","rows":5,"cols":7},
                {"id":"small_pouch","name":"小口袋","rows":3,"cols":3},
                {"id":"front_satchel","name":"前挂包","rows":3,"cols":4}
              ],
              "placed_items": [
                {
                  "container_id": "small_pouch",
                  "row": 2,
                  "col": 2,
                  "item": {
                    "instance_id": 1001,
                    "item_id": "starter_talisman",
                    "display_name": "启程护符",
                    "grid_width": 2,
                    "grid_height": 2,
                    "weight": 0.2,
                    "rarity": "uncommon",
                    "description": "初入修途者配发的护身符。",
                    "stack_count": 1,
                    "spirit_quality": 0.76,
                    "durability": 0.93
                  }
                }
              ],
              "equipped": {
                "head": null,
                "chest": null,
                "legs": null,
                "feet": null,
                "main_hand": null,
                "off_hand": null,
                "two_hand": null
              },
              "hotbar": [null, null, null, null, null, null, null, null, null],
              "bone_coins": 57,
              "weight": {"current": 0.2, "max": 50.0},
              "realm": "qi_refining_1",
              "qi_current": 24,
              "qi_max": 100,
              "body_level": 0.18
            }
            """;

        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            overflow,
            overflow.getBytes(StandardCharsets.UTF_8).length
        );
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());

        ServerDataDispatch dispatch = new InventorySnapshotHandler().handle(parseResult.envelope());
        assertFalse(dispatch.handled());
        assertTrue(dispatch.logMessage().contains("placed_items"));
        assertFalse(InventoryStateStore.isAuthoritativeLoaded());
        assertEquals(-1L, InventoryStateStore.revision());
        assertTrue(InventoryStateStore.snapshot().isEmpty());
    }

    @Test
    void parsesScrollMetadataAcrossSkillRecipeAndBlueprintKinds() {
        String snapshotJson = """
            {
              "v": 1,
              "type": "inventory_snapshot",
              "revision": 42,
              "containers": [
                {"id":"main_pack","name":"主背包","rows":5,"cols":7},
                {"id":"small_pouch","name":"小口袋","rows":3,"cols":3},
                {"id":"front_satchel","name":"前挂包","rows":3,"cols":4}
              ],
              "placed_items": [
                {
                  "container_id": "main_pack",
                  "row": 0,
                  "col": 0,
                  "item": {
                    "instance_id": 2001,
                    "item_id": "skill_scroll_herbalism_baicao_can",
                    "display_name": "百草残卷",
                    "grid_width": 1,
                    "grid_height": 1,
                    "weight": 0.1,
                    "rarity": "rare",
                    "description": "可悟 Herbalism 的 skill 残卷。",
                    "stack_count": 1,
                    "spirit_quality": 0.9,
                    "durability": 1.0,
                    "scroll_kind": "skill_scroll",
                    "scroll_skill_id": "herbalism",
                    "scroll_xp_grant": 500
                  }
                },
                {
                  "container_id": "main_pack",
                  "row": 0,
                  "col": 1,
                  "item": {
                    "instance_id": 2002,
                    "item_id": "recipe_scroll_qixue_pill",
                    "display_name": "丹方残卷·气血丹",
                    "grid_width": 1,
                    "grid_height": 1,
                    "weight": 0.05,
                    "rarity": "uncommon",
                    "description": "炼丹丹方残卷。",
                    "stack_count": 1,
                    "spirit_quality": 1.0,
                    "durability": 1.0,
                    "scroll_kind": "recipe_scroll"
                  }
                },
                {
                  "container_id": "main_pack",
                  "row": 0,
                  "col": 2,
                  "item": {
                    "instance_id": 2003,
                    "item_id": "blueprint_scroll_bronze_tripod",
                    "display_name": "器图残卷·青铜鼎",
                    "grid_width": 1,
                    "grid_height": 1,
                    "weight": 0.08,
                    "rarity": "rare",
                    "description": "锻造器图残卷。",
                    "stack_count": 1,
                    "spirit_quality": 1.0,
                    "durability": 1.0,
                    "scroll_kind": "blueprint_scroll"
                  }
                }
              ],
              "equipped": {
                "head": null,
                "chest": null,
                "legs": null,
                "feet": null,
                "main_hand": null,
                "off_hand": null,
                "two_hand": null
              },
              "hotbar": [null, null, null, null, null, null, null, null, null],
              "bone_coins": 0,
              "weight": {"current": 0.23, "max": 50.0},
              "realm": "qi_refining_1",
              "qi_current": 24,
              "qi_max": 100,
              "body_level": 0.18
            }
            """;

        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            snapshotJson,
            snapshotJson.getBytes(StandardCharsets.UTF_8).length
        );
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());

        ServerDataDispatch dispatch = new InventorySnapshotHandler().handle(parseResult.envelope());
        assertTrue(dispatch.handled(), dispatch.logMessage());

        InventoryModel snapshot = InventoryStateStore.snapshot();
        assertEquals(3, snapshot.gridItems().size());

        InventoryItem skillScroll = snapshot.gridItems().get(0).item();
        assertEquals("skill_scroll", skillScroll.scrollKind());
        assertEquals("herbalism", skillScroll.scrollSkillId());
        assertEquals(500, skillScroll.scrollXpGrant());
        assertTrue(skillScroll.isSkillScroll());

        InventoryItem recipeScroll = snapshot.gridItems().get(1).item();
        assertEquals("recipe_scroll", recipeScroll.scrollKind());
        assertEquals("", recipeScroll.scrollSkillId());
        assertEquals(0, recipeScroll.scrollXpGrant());
        assertFalse(recipeScroll.isSkillScroll());

        InventoryItem blueprintScroll = snapshot.gridItems().get(2).item();
        assertEquals("blueprint_scroll", blueprintScroll.scrollKind());
        assertEquals("", blueprintScroll.scrollSkillId());
        assertEquals(0, blueprintScroll.scrollXpGrant());
        assertFalse(blueprintScroll.isSkillScroll());
    }

    private static String loadSharedFixture(String fileName) throws IOException {
        Path fixturePath = SHARED_SCHEMA_SAMPLES_DIR.resolve(fileName);
        return Files.readString(fixturePath, StandardCharsets.UTF_8);
    }
}
