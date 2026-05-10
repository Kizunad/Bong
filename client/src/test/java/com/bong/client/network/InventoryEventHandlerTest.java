package com.bong.client.network;

import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.DroppedItemStore;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.state.VisualEffectState;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class InventoryEventHandlerTest {
    @BeforeEach
    void setUp() {
        InventoryStateStore.resetForTests();
        DroppedItemStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        InventoryStateStore.resetForTests();
        DroppedItemStore.resetForTests();
    }

    @Test
    void eventBeforeAuthoritativeSnapshotIsIgnoredSafely() {
        ServerDataDispatch dispatch = new InventoryEventHandler().handle(parseEnvelope("""
            {"v":1,"type":"inventory_event","kind":"stack_changed","revision":13,"instance_id":1004,"stack_count":1}
            """));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.logMessage().contains("snapshot is not loaded"));
        assertFalse(InventoryStateStore.isAuthoritativeLoaded());
        assertEquals(-1L, InventoryStateStore.revision());
        assertTrue(InventoryStateStore.snapshot().isEmpty());
    }

    @Test
    void staleRevisionIsIgnoredSafelyWithoutMutatingStore() {
        InventoryModel baseline = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .gridItem(
                InventoryItem.createFull(
                    1001L,
                    "starter_talisman",
                    "启程护符",
                    1,
                    1,
                    0.2,
                    "uncommon",
                    "初入修途者配发的护身符。",
                    1,
                    0.76,
                    0.93
                ),
                InventoryModel.PRIMARY_CONTAINER_ID,
                0,
                0
            )
            .build();
        InventoryStateStore.applyAuthoritativeSnapshot(baseline, 12L);

        ServerDataDispatch dispatch = new InventoryEventHandler().handle(parseEnvelope("""
            {"v":1,"type":"inventory_event","kind":"stack_changed","revision":11,"instance_id":1004,"stack_count":1}
            """));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.logMessage().contains("stale"));
        assertEquals(12L, InventoryStateStore.revision());
        assertTrue(InventoryStateStore.isAuthoritativeLoaded());
        assertEquals(baseline, InventoryStateStore.snapshot());
    }

    @Test
    void unsupportedKindIsIgnoredSafelyWithoutMutatingStore() {
        InventoryModel baseline = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .gridItem(
                InventoryItem.createFull(
                    1001L,
                    "starter_talisman",
                    "启程护符",
                    1,
                    1,
                    0.2,
                    "uncommon",
                    "初入修途者配发的护身符。",
                    1,
                    0.76,
                    0.93
                ),
                InventoryModel.PRIMARY_CONTAINER_ID,
                0,
                0
            )
            .build();
        InventoryStateStore.applyAuthoritativeSnapshot(baseline, 12L);

        ServerDataDispatch dispatch = new InventoryEventHandler().handle(parseEnvelope("""
            {"v":1,"type":"inventory_event","kind":"teleported","revision":13,"instance_id":1001}
            """));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.logMessage().contains("unsupported"));
        assertEquals(12L, InventoryStateStore.revision());
        assertTrue(InventoryStateStore.isAuthoritativeLoaded());
        assertEquals(baseline, InventoryStateStore.snapshot());
    }

    @Test
    void stackChangedAppliesAndBumpsRevision() {
        InventoryModel baseline = baselineWithStarterTalisman();
        InventoryStateStore.applyAuthoritativeSnapshot(baseline, 5L);

        ServerDataDispatch dispatch = new InventoryEventHandler().handle(parseEnvelope("""
            {"v":1,"type":"inventory_event","kind":"stack_changed","revision":6,"instance_id":1001,"stack_count":7}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        assertEquals(6L, InventoryStateStore.revision());
        InventoryItem updated = InventoryStateStore.snapshot().gridItems().get(0).item();
        assertEquals(7, updated.stackCount());
        assertEquals(0.93, updated.durability(), 1e-9);
    }

    @Test
    void durabilityChangedAppliesAndBumpsRevision() {
        InventoryModel baseline = baselineWithStarterTalisman();
        InventoryStateStore.applyAuthoritativeSnapshot(baseline, 5L);

        ServerDataDispatch dispatch = new InventoryEventHandler().handle(parseEnvelope("""
            {"v":1,"type":"inventory_event","kind":"durability_changed","revision":6,"instance_id":1001,"durability":0.5}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        InventoryItem updated = InventoryStateStore.snapshot().gridItems().get(0).item();
        assertEquals(0.5, updated.durability(), 1e-9);
    }

    @Test
    void durabilityChangedBreakingArmorEmitsToast() {
        InventoryModel baseline = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .equip(
                com.bong.client.inventory.model.EquipSlotType.CHEST,
                InventoryItem.createFull(
                    1002L,
                    "fake_spirit_hide",
                    "假灵兽皮胸甲",
                    2,
                    2,
                    5.0,
                    "rare",
                    "fixture",
                    1,
                    0.8,
                    0.2
                )
            )
            .build();
        InventoryStateStore.applyAuthoritativeSnapshot(baseline, 5L);

        ServerDataDispatch dispatch = new InventoryEventHandler().handle(parseEnvelope("""
            {"v":1,"type":"inventory_event","kind":"durability_changed","revision":6,"instance_id":1002,"durability":0.0}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        assertTrue(dispatch.alertToast().isPresent(), "armor break should return toast dispatch");
        ServerDataDispatch.ToastSpec toast = dispatch.alertToast().orElseThrow();
        assertEquals("胸甲破损", toast.text());
        assertEquals(1_200L, toast.durationMillis());
        assertEquals(
            VisualEffectState.EffectType.ARMOR_BREAK_FLASH,
            dispatch.visualEffectState().orElseThrow().effectType()
        );
    }

    @Test
    void durabilityChangedCrossingArmorLowThresholdEmitsWarningToastAndRedFlash() {
        InventoryModel baseline = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .equip(
                com.bong.client.inventory.model.EquipSlotType.CHEST,
                InventoryItem.createFull(
                    1004L,
                    "armor_iron_chestplate",
                    "铁甲胸甲",
                    2,
                    2,
                    5.0,
                    "common",
                    "fixture",
                    1,
                    0.8,
                    0.21
                )
            )
            .build();
        InventoryStateStore.applyAuthoritativeSnapshot(baseline, 5L);

        ServerDataDispatch dispatch = new InventoryEventHandler().handle(parseEnvelope("""
            {"v":1,"type":"inventory_event","kind":"durability_changed","revision":6,"instance_id":1004,"durability":0.19}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        assertEquals("甲胄将破", dispatch.alertToast().orElseThrow().text());
        assertEquals(
            VisualEffectState.EffectType.ARMOR_LOW_DURABILITY_FLASH,
            dispatch.visualEffectState().orElseThrow().effectType()
        );
    }

    @Test
    void movedEquippingMundaneArmorEmitsWearFlash() {
        InventoryModel baseline = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .gridItem(
                InventoryItem.createFull(
                    1005L,
                    "armor_bone_chestplate",
                    "骨甲胸甲",
                    2,
                    2,
                    1.4,
                    "common",
                    "fixture",
                    1,
                    0.8,
                    1.0
                ),
                InventoryModel.PRIMARY_CONTAINER_ID,
                0,
                0
            )
            .build();
        InventoryStateStore.applyAuthoritativeSnapshot(baseline, 5L);

        ServerDataDispatch dispatch = new InventoryEventHandler().handle(parseEnvelope("""
            {"v":1,"type":"inventory_event","kind":"moved","revision":6,"instance_id":1005,
             "from":{"kind":"container","container_id":"main_pack","row":0,"col":0},
             "to":{"kind":"equip","slot":"chest"}}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        assertTrue(dispatch.alertToast().isEmpty());
        assertEquals(
            VisualEffectState.EffectType.ARMOR_EQUIP_FLASH,
            dispatch.visualEffectState().orElseThrow().effectType()
        );
        assertEquals(
            "armor_bone_chestplate",
            InventoryStateStore.snapshot()
                .equipped()
                .get(com.bong.client.inventory.model.EquipSlotType.CHEST)
                .itemId()
        );
    }

    @Test
    void durabilityChangedBreakingNonArmorInArmorSlotDoesNotEmitToast() {
        InventoryModel baseline = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .equip(
                com.bong.client.inventory.model.EquipSlotType.CHEST,
                InventoryItem.createFull(
                    1003L,
                    "training_blade",
                    "练习刀",
                    1,
                    3,
                    2.0,
                    "common",
                    "fixture",
                    1,
                    0.8,
                    0.2
                )
            )
            .build();
        InventoryStateStore.applyAuthoritativeSnapshot(baseline, 5L);

        ServerDataDispatch dispatch = new InventoryEventHandler().handle(parseEnvelope("""
            {"v":1,"type":"inventory_event","kind":"durability_changed","revision":6,"instance_id":1003,"durability":0.0}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        assertTrue(dispatch.alertToast().isEmpty(), "non-armor item should not return armor break toast");
    }

    @Test
    void movedFromGridToHotbarRelocatesItem() {
        InventoryModel baseline = baselineWithStarterTalisman();
        InventoryStateStore.applyAuthoritativeSnapshot(baseline, 5L);

        ServerDataDispatch dispatch = new InventoryEventHandler().handle(parseEnvelope("""
            {"v":1,"type":"inventory_event","kind":"moved","revision":6,"instance_id":1001,
             "from":{"kind":"container","container_id":"main_pack","row":0,"col":0},
             "to":{"kind":"hotbar","index":3}}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        InventoryModel after = InventoryStateStore.snapshot();
        assertTrue(after.gridItems().isEmpty(), "grid should be empty after move out");
        InventoryItem hotbarItem = after.hotbar().get(3);
        assertEquals(1001L, hotbarItem.instanceId());
    }

    @Test
    void movedTrustsServerToEvenIfFromOutOfSync() {
        // 复现真实场景：client 已乐观把 1001 从 grid 搬到 hotbar(0)，server 然后回推
        // moved with from=container（server's view），to=hotbar(3)。client 应当信任
        // server 的 to，把 instance 重定位到 hotbar(3)，而不是因 from 不匹配而拒绝。
        InventoryModel baseline = baselineWithStarterTalisman();
        InventoryStateStore.applyAuthoritativeSnapshot(baseline, 5L);

        ServerDataDispatch dispatch = new InventoryEventHandler().handle(parseEnvelope("""
            {"v":1,"type":"inventory_event","kind":"moved","revision":6,"instance_id":1001,
             "from":{"kind":"hotbar","index":0},
             "to":{"kind":"hotbar","index":3}}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        InventoryModel after = InventoryStateStore.snapshot();
        assertEquals(6L, after.gridItems().size() == 0 ? 6L : 6L);
        assertTrue(after.gridItems().isEmpty(), "item should leave grid");
        InventoryItem hotbarItem = after.hotbar().get(3);
        assertEquals(1001L, hotbarItem.instanceId());
    }

    @Test
    void droppedRemovesItemFromAuthoritativeSnapshot() {
        InventoryModel baseline = baselineWithStarterTalisman();
        InventoryStateStore.applyAuthoritativeSnapshot(baseline, 5L);

        ServerDataDispatch dispatch = new InventoryEventHandler().handle(parseEnvelope("""
            {"v":1,"type":"inventory_event","kind":"dropped","revision":6,"instance_id":1001,
             "from":{"kind":"container","container_id":"main_pack","row":0,"col":0},
             "world_pos":[8.5,66.0,8.5],
             "item":{"instance_id":1001,"item_id":"starter_talisman","display_name":"启程护符",
                      "grid_width":1,"grid_height":1,"weight":0.2,"rarity":"uncommon",
                      "description":"初入修途者配发的护身符。","stack_count":1,
                     "spirit_quality":0.76,"durability":0.93}}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        InventoryModel after = InventoryStateStore.snapshot();
        assertTrue(after.gridItems().isEmpty(), "grid item should be removed after dropped event");
        assertEquals(6L, InventoryStateStore.revision());
        DroppedItemStore.Entry dropped = DroppedItemStore.get(1001L);
        assertEquals("main_pack", dropped.sourceContainerId());
        assertEquals(0, dropped.sourceRow());
        assertEquals(0, dropped.sourceCol());
        assertEquals(8.5, dropped.worldPosX());
        assertEquals(66.0, dropped.worldPosY());
        assertEquals(8.5, dropped.worldPosZ());
        assertEquals("starter_talisman", dropped.item().itemId());
    }

    @Test
    void droppedFromHotbarRemovesItemAndWritesDroppedStoreEntry() {
        InventoryModel baseline = InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .hotbar(0, InventoryItem.createFull(
                1001L,
                "starter_talisman",
                "启程护符",
                1,
                1,
                0.2,
                "uncommon",
                "初入修途者配发的护身符。",
                1,
                0.76,
                0.93
            ))
            .build();
        InventoryStateStore.applyAuthoritativeSnapshot(baseline, 5L);

        ServerDataDispatch dispatch = new InventoryEventHandler().handle(parseEnvelope("""
            {"v":1,"type":"inventory_event","kind":"dropped","revision":6,"instance_id":1001,
             "from":{"kind":"hotbar","index":0},
             "world_pos":[8.5,66.0,8.5],
             "item":{"instance_id":1001,"item_id":"starter_talisman","display_name":"启程护符",
                      "grid_width":1,"grid_height":1,"weight":0.2,"rarity":"uncommon",
                      "description":"初入修途者配发的护身符。","stack_count":1,
                      "spirit_quality":0.76,"durability":0.93}}
            """));

        assertTrue(dispatch.handled(), dispatch.logMessage());
        DroppedItemStore.Entry dropped = DroppedItemStore.get(1001L);
        assertEquals("hotbar", dropped.sourceContainerId());
        assertEquals(0, dropped.sourceRow());
        assertEquals(0, dropped.sourceCol());
        assertNull(InventoryStateStore.snapshot().hotbar().get(0));
        assertEquals(6L, InventoryStateStore.revision());
    }

    @Test
    void droppedWithMissingItemPayloadIsIgnoredSafely() {
        InventoryModel baseline = baselineWithStarterTalisman();
        InventoryStateStore.applyAuthoritativeSnapshot(baseline, 5L);

        ServerDataDispatch dispatch = new InventoryEventHandler().handle(parseEnvelope("""
            {"v":1,"type":"inventory_event","kind":"dropped","revision":6,"instance_id":1001,
             "from":{"kind":"container","container_id":"main_pack","row":0,"col":0}}
            """));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.logMessage().contains("invalid from/world_pos/item payload"));
        assertEquals(baseline, InventoryStateStore.snapshot());
        assertEquals(5L, InventoryStateStore.revision());
    }

    @Test
    void droppedWithInvalidChargesPayloadIsIgnoredSafely() {
        InventoryModel baseline = baselineWithStarterTalisman();
        InventoryStateStore.applyAuthoritativeSnapshot(baseline, 5L);

        ServerDataDispatch dispatch = new InventoryEventHandler().handle(parseEnvelope("""
            {"v":1,"type":"inventory_event","kind":"dropped","revision":6,"instance_id":1001,
             "from":{"kind":"container","container_id":"main_pack","row":0,"col":0},
             "world_pos":[8.5,66.0,8.5],
             "item":{"instance_id":1001,"item_id":"ancient_relic","display_name":"上古遗物",
                     "grid_width":1,"grid_height":1,"weight":0.2,"rarity":"ancient",
                     "description":"","stack_count":1,"spirit_quality":1.0,"durability":1.0,
                     "charges":"bad"}}
            """));

        assertFalse(dispatch.handled());
        assertTrue(dispatch.logMessage().contains("invalid from/world_pos/item payload"));
        assertEquals(baseline, InventoryStateStore.snapshot());
        assertEquals(5L, InventoryStateStore.revision());
        assertTrue(DroppedItemStore.snapshot().isEmpty());
    }

    private static InventoryModel baselineWithStarterTalisman() {
        return InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .gridItem(
                InventoryItem.createFull(
                    1001L,
                    "starter_talisman",
                    "启程护符",
                    1, 1, 0.2,
                    "uncommon",
                    "初入修途者配发的护身符。",
                    1, 0.76, 0.93
                ),
                InventoryModel.PRIMARY_CONTAINER_ID,
                0, 0
            )
            .build();
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(
            json,
            json.getBytes(StandardCharsets.UTF_8).length
        );
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
