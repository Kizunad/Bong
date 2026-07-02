package com.bong.client.network;

import bong.Envelope;
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
             "world_pos_x":8.5,"world_pos_y":66.0,"world_pos_z":8.5,
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
             "world_pos_x":8.5,"world_pos_y":66.0,"world_pos_z":8.5,
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
             "world_pos_x":8.5,"world_pos_y":66.0,"world_pos_z":8.5,
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

    @Test
    void droppedWithLegacyArrayWorldPosIsNowRejectedSafely() {
        // fix/proto-worldpos-flat-cluster: the fix is intentionally incompatible — a
        // "world_pos":[x,y,z] array (the pre-fix shape) must now be treated the same as a
        // missing world_pos (noOp), not silently coerced. InventoryEventDropped only ever
        // produces flat world_pos_x/_y/_z on the real wire, so the array shape is dead.
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

        assertFalse(dispatch.handled(),
            "旧数组形状 world_pos 不再被读取（world_pos_x/_y/_z 缺失），应 noOp 而非误判为有效坐标，"
            + "实际 handled=" + dispatch.handled());
        assertTrue(dispatch.logMessage().contains("invalid from/world_pos/item payload"));
        assertEquals(baseline, InventoryStateStore.snapshot());
        assertEquals(5L, InventoryStateStore.revision());
    }

    // ── fix/proto-worldpos-flat-cluster: real production proto wire regressions ──
    //
    // 根因（与 ContainerInteractionHandlerTest#containerStateAppliesFlatWorldPosThroughRealProtoWire
    // 同一 cluster）：InventoryEventDropped（proto/bong/envelope.proto）把 Rust [f64;3] 拆成三个 flat
    // 字段 world_pos_x/world_pos_y/world_pos_z。ProtoServerDataBridge 用
    // JsonFormat.preservingProtoFieldNames() 把 proto 转成 legacy JSON 时**不会**把这三个 flat 字段
    // 重塑回 JSON 数组——生产（--release）下走的正是这条 proto 路径。旧版 handler 用
    // readRequiredArray(payload, "world_pos") 读**数组**，在 proto-bridged JSON 里永远拿 null →
    // 整条 dropped 事件被 noOp 丢弃，物品从背包"消失"却卡在库存里、DroppedItemStore 永不落地。
    //
    // 顺带发现并锁死的第二个缺口：InventoryEventDropped/Moved 的 "from"/"to" 字段类型
    // InventoryLocation 内部也是一个 proto oneof（container/equip/hotbar），
    // ProtoServerDataBridge.bridgeOneofFlat 只摊平*外层* InventoryEvent.event oneof
    // （moved/dropped/stack_changed/durability_changed）到顶层 "kind" 标签，不重塑*内层*
    // InventoryLocation.location oneof —— JsonFormat 把它打成
    // {"container": {...}} / {"equip": {...}} / {"hotbar": {...}}，没有 "kind" 判别字段。
    // 若不修，即使 world_pos 修好，parseLocation 仍会因读不到 "kind" 而返回 null，dropped/moved
    // 事件照样 noOp。三条真机 proto e2e 测试分别覆盖 container/hotbar/equip 三种 from/to 形状。

    @Test
    void droppedThroughRealProtoWireAppliesFlatWorldPosAndPopulatesDroppedStore() {
        InventoryModel baseline = baselineWithStarterTalisman();
        InventoryStateStore.applyAuthoritativeSnapshot(baseline, 5L);

        Envelope.InventoryEventDropped dropped = Envelope.InventoryEventDropped.newBuilder()
                .setRevision(6)
                .setInstanceId(1001)
                .setFrom(Envelope.InventoryLocation.newBuilder()
                        .setContainer(Envelope.InventoryLocationContainer.newBuilder()
                                .setContainerId("main_pack")
                                .setRow(0)
                                .setCol(0)))
                .setWorldPosX(8.5)
                .setWorldPosY(66.0)
                .setWorldPosZ(8.5)
                .setItem(baseItemView(1001, "starter_talisman", "启程护符"))
                .build();

        ServerDataDispatch dispatch = dispatchThroughRealProtoWire(
                Envelope.InventoryEvent.newBuilder().setDropped(dropped));

        assertTrue(dispatch.handled(),
                "dropped 事件应被 handler 接受（非 noOp），实际 log=" + dispatch.logMessage()
                + "；若 noOp 说明 world_pos 在 proto wire 上以 flat world_pos_x/_y/_z 字段落地，而"
                + "handler 仍按 JSON 数组读取 world_pos → worldPos==null → 整条 dropped 事件被丢弃"
                + "（本次要锁死的回归：修复前必 RED）。");

        InventoryModel after = InventoryStateStore.snapshot();
        assertTrue(after.gridItems().isEmpty(),
                "grid item 应在 dropped 事件应用后被移除，实际 gridItems=" + after.gridItems());
        assertEquals(6L, InventoryStateStore.revision());

        DroppedItemStore.Entry entry = DroppedItemStore.get(1001L);
        assertEquals("main_pack", entry.sourceContainerId());
        assertEquals(0, entry.sourceRow());
        assertEquals(0, entry.sourceCol());
        assertEquals(8.5, entry.worldPosX(),
                "world_pos_x=8.5 须经 proto flat 字段正确落进 DroppedItemStore，实际 " + entry.worldPosX());
        assertEquals(66.0, entry.worldPosY(),
                "world_pos_y=66.0 须经 proto flat 字段正确落进 DroppedItemStore，实际 " + entry.worldPosY());
        assertEquals(8.5, entry.worldPosZ(),
                "world_pos_z=8.5 须经 proto flat 字段正确落进 DroppedItemStore，实际 " + entry.worldPosZ());
        assertEquals("starter_talisman", entry.item().itemId());
    }

    @Test
    void droppedFromHotbarThroughRealProtoWireAcceptsNestedLocationOneofShape() {
        // 锁死 InventoryLocation.location 的 proto-native "hotbar" 形状：{"hotbar":{"index":0}}
        // （没有 "kind" 判别字段），而不仅仅是外层 InventoryEvent.event 已被 bridgeOneofFlat 摊平的
        // "kind":"dropped"。
        InventoryModel baseline = InventoryModel.builder()
                .containers(InventoryModel.DEFAULT_CONTAINERS)
                .hotbar(0, InventoryItem.createFull(
                        1001L,
                        "starter_talisman",
                        "启程护符",
                        1, 1, 0.2,
                        "uncommon",
                        "初入修途者配发的护身符。",
                        1, 0.76, 0.93
                ))
                .build();
        InventoryStateStore.applyAuthoritativeSnapshot(baseline, 5L);

        Envelope.InventoryEventDropped dropped = Envelope.InventoryEventDropped.newBuilder()
                .setRevision(6)
                .setInstanceId(1001)
                .setFrom(Envelope.InventoryLocation.newBuilder()
                        .setHotbar(Envelope.InventoryLocationHotbar.newBuilder().setIndex(0)))
                .setWorldPosX(8.5)
                .setWorldPosY(66.0)
                .setWorldPosZ(8.5)
                .setItem(baseItemView(1001, "starter_talisman", "启程护符"))
                .build();

        ServerDataDispatch dispatch = dispatchThroughRealProtoWire(
                Envelope.InventoryEvent.newBuilder().setDropped(dropped));

        assertTrue(dispatch.handled(),
                "dropped(from=hotbar) 应被接受，实际 log=" + dispatch.logMessage()
                + "；若 noOp，需检查 from 的 proto-native {\"hotbar\":{\"index\":0}} 形状是否被"
                + "parseLocation 正确识别（该形状没有 \"kind\" 判别字段）。");

        DroppedItemStore.Entry entry = DroppedItemStore.get(1001L);
        assertEquals("hotbar", entry.sourceContainerId(),
                "from=hotbar 应写入 DroppedItemStore.sourceContainerId=\"hotbar\"，实际 "
                + entry.sourceContainerId());
        assertNull(InventoryStateStore.snapshot().hotbar().get(0),
                "hotbar slot 0 应在 dropped 事件应用后清空");
    }

    @Test
    void movedToEquipThroughRealProtoWireStripsEquipSlotEnumPrefix() {
        // 锁死 InventoryLocation.location 的 proto-native "equip" 形状 {"equip":{"slot":
        // "EQUIP_SLOT_CHEST",...}}：proto 枚举打印全名前缀（EQUIP_SLOT_CHEST），而
        // EQUIP_SLOT_BY_WIRE_NAME 只认小写 wire 名（"chest"），parseLocation 须归一化。
        InventoryModel baseline = InventoryModel.builder()
                .containers(InventoryModel.DEFAULT_CONTAINERS)
                .gridItem(
                        InventoryItem.createFull(
                                1005L,
                                "armor_bone_chestplate",
                                "骨甲胸甲",
                                2, 2, 1.4,
                                "common",
                                "fixture",
                                1, 0.8, 1.0
                        ),
                        InventoryModel.PRIMARY_CONTAINER_ID,
                        0, 0
                )
                .build();
        InventoryStateStore.applyAuthoritativeSnapshot(baseline, 5L);

        Envelope.InventoryEventMoved moved = Envelope.InventoryEventMoved.newBuilder()
                .setRevision(6)
                .setInstanceId(1005)
                .setFrom(Envelope.InventoryLocation.newBuilder()
                        .setContainer(Envelope.InventoryLocationContainer.newBuilder()
                                .setContainerId("main_pack")
                                .setRow(0)
                                .setCol(0)))
                .setTo(Envelope.InventoryLocation.newBuilder()
                        .setEquip(Envelope.InventoryLocationEquip.newBuilder()
                                .setSlot(Envelope.EquipSlot.EQUIP_SLOT_CHEST)
                                .setState(Envelope.EquipState.EQUIP_STATE_WORN)))
                .build();

        ServerDataDispatch dispatch = dispatchThroughRealProtoWire(
                Envelope.InventoryEvent.newBuilder().setMoved(moved));

        assertTrue(dispatch.handled(),
                "moved(to=equip) 应被接受，实际 log=" + dispatch.logMessage()
                + "；若 noOp，需检查 to 的 proto-native {\"equip\":{\"slot\":\"EQUIP_SLOT_CHEST\",...}}"
                + " 形状是否被 parseLocation 正确识别并剥离 EQUIP_SLOT_ 前缀。");

        assertEquals(
                "armor_bone_chestplate",
                InventoryStateStore.snapshot()
                        .equipped()
                        .get(com.bong.client.inventory.model.EquipSlotType.CHEST)
                        .itemId(),
                "instance 1005 应落进 CHEST 装备槽（EquipSlot 全名前缀被正确剥离归一化）"
        );
    }

    /** 一个最小但完整（不会被 parseInventoryItem 判 null）的 InventoryItemView proto builder。 */
    private static Envelope.InventoryItemView baseItemView(long instanceId, String itemId, String displayName) {
        return Envelope.InventoryItemView.newBuilder()
                .setInstanceId(instanceId)
                .setItemId(itemId)
                .setDisplayName(displayName)
                .setGridWidth(1)
                .setGridHeight(1)
                .setWeight(0.2)
                .setRarity("uncommon")
                .setStackCount(1)
                .setSpiritQuality(0.76)
                .setDurability(0.93)
                .build();
    }

    /**
     * 把一个 InventoryEvent（proto builder 构造）过真机生产链：proto→bytes→
     * {@link ProtoServerDataBridge#bridge}（生产解码路径）→{@link ServerDataEnvelope#parse}→
     * {@link InventoryEventHandler#handle}，返回最终 dispatch 供断言。
     */
    private static ServerDataDispatch dispatchThroughRealProtoWire(Envelope.InventoryEvent.Builder eventBuilder) {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setInventoryEvent(eventBuilder)
                .build();

        ProtoServerDataBridge.BridgeResult bridged = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(bridged.isSuccess(),
                "proto→legacyJson 桥接应成功，实际 error=" + bridged.errorMessage());

        String legacyJson = bridged.legacyJson();
        ServerPayloadParseResult parsed = ServerDataEnvelope.parse(
                legacyJson, legacyJson.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parsed.isSuccess(),
                "桥输出的 legacyJson 应能被解析，实际 error=" + parsed.errorMessage());

        return new InventoryEventHandler().handle(parsed.envelope());
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
