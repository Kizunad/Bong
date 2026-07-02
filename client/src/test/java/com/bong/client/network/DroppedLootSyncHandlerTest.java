package com.bong.client.network;

import bong.Envelope;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.state.DroppedItemStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * dropped_loot_sync 端到端回归。
 *
 * <p><b>根因（fix/dropped-loot-world-pos-flat）</b>：生产 {@code --release} 走 protobuf
 * （{@code agent_bridge} cfg(not(test))）。proto {@code DroppedLootEntry} 把 Rust {@code [f64;3]}
 * 拆成 flat {@code world_pos_x/y/z} 三字段，经 {@link ProtoServerDataBridge}
 * （{@code preservingProtoFieldNames}）展平后 client 收到的就是这三个 key，<b>不是</b> {@code world_pos}
 * 数组。{@link DroppedLootSyncHandler} 早期只读数组 → proto 路径每条 entry 被拒 →
 * {@link DroppedItemStore} 永不填充 → 从 InventoryUI 丢弃的物品在世界里没有任何 icon
 * （{@code DroppedItemWorldRenderer} 无内容可渲染）。旧单测吃数组 JSON 走假路径永远抓不到。
 *
 * <p>因此本类的 happy-path 用例{@link #droppedLootSyncSurvivesRealProtoWire 走真机生产链}
 * （真构 proto 字节 → {@code bridge} → router → store），修复前必 RED（store size 0）。
 */
public class DroppedLootSyncHandlerTest {

    @AfterEach
    void tearDown() {
        DroppedItemStore.resetForTests();
    }

    // ── happy path：真机 proto 生产链 ──

    @Test
    void droppedLootSyncSurvivesRealProtoWire() {
        Envelope.DroppedLootSync sync = Envelope.DroppedLootSync.newBuilder()
                .addDrops(Envelope.DroppedLootEntry.newBuilder()
                        .setInstanceId(1004L)
                        .setSourceContainerId("main_pack")
                        .setSourceRow(0)
                        .setSourceCol(0)
                        .setWorldPosX(8.5)
                        .setWorldPosY(66.0)
                        .setWorldPosZ(-4.25)
                        .setItem(baseItemView(1004L, "starter_talisman", "启程护符")))
                .build();

        routeProtoWire(sync);

        assertEquals(1, DroppedItemStore.snapshot().size(),
                "proto wire 的掉落 entry 必须被解析进 store；size=0 说明 world_pos flat/array 形状不匹配回归了");
        DroppedItemStore.Entry entry = DroppedItemStore.get(1004L);
        assertNotNull(entry, "instance 1004 应在 store 中（proto wire 解析成功）");
        assertEquals("main_pack", entry.sourceContainerId());
        assertEquals(8.5, entry.worldPosX(), "world_pos_x 须经 proto wire (flat) 存活");
        assertEquals(66.0, entry.worldPosY(), "world_pos_y 须经 proto wire (flat) 存活");
        assertEquals(-4.25, entry.worldPosZ(), "world_pos_z 须经 proto wire (flat) 存活");
        assertEquals("starter_talisman", entry.item().itemId());
    }

    @Test
    void multipleDropsSurviveProtoWireWithDistinctPositions() {
        Envelope.DroppedLootSync sync = Envelope.DroppedLootSync.newBuilder()
                .addDrops(dropEntry(2001L, "main_pack", 1.0, 64.0, 2.0,
                        baseItemView(2001L, "gu_bi", "骨币")))
                .addDrops(dropEntry(2002L, "body_pocket", -10.5, 70.0, 33.25,
                        baseItemView(2002L, "starter_talisman", "启程护符")))
                .build();

        routeProtoWire(sync);

        assertEquals(2, DroppedItemStore.snapshot().size(), "两条掉落都须进 store");
        assertEquals(2.0, DroppedItemStore.get(2001L).worldPosZ(), "entry 2001 坐标不应串味");
        assertEquals(-10.5, DroppedItemStore.get(2002L).worldPosX(), "entry 2002 坐标不应串味");
    }

    @Test
    void ancientChargesSurviveProtoWire() {
        Envelope.DroppedLootSync sync = Envelope.DroppedLootSync.newBuilder()
                .addDrops(dropEntry(9001L, "main_pack", 1.0, 64.0, 2.0,
                        baseItemView(9001L, "ancient_relic", "上古遗物")
                                .setRarity("ancient")
                                .setCharges(3)))
                .build();

        routeProtoWire(sync);

        DroppedItemStore.Entry entry = DroppedItemStore.get(9001L);
        assertNotNull(entry, "ancient 掉落须进 store");
        assertEquals(3, entry.item().charges(), "ancient charges=3 须经 proto wire 存活");
    }

    // ── 空 sync：清空 store ──

    @Test
    void emptyDroppedLootSyncClearsExistingStore() {
        DroppedItemStore.putOrReplace(new DroppedItemStore.Entry(
                1004L, "main_pack", 0, 0, 8.5, 66.0, 8.5,
                InventoryItem.simple("starter_talisman", "启程护符")));

        routeProtoWire(Envelope.DroppedLootSync.newBuilder().build());

        assertTrue(DroppedItemStore.snapshot().isEmpty(),
                "空 drops 的 sync 必须清空 store（掉落物被别人捡走 / 塌缩后世界里应消失）");
    }

    // ── 边界：非法 entry 被拒且不抛（flat wire 形状） ──

    @Test
    void malformedDroppedLootPositionIsRejectedWithoutThrowing() {
        // flat wire 缺 world_pos_z → entry 非法 → 拒绝整份 payload 且不抛异常。
        String payload = """
            {"v":1,"type":"dropped_loot_sync","drops":[
              {"instance_id":9002,"source_container_id":"main_pack","source_row":0,"source_col":0,
               "world_pos_x":1.0,"world_pos_y":64.0,
               "item":{"instance_id":9002,"item_id":"rare_relic","display_name":"稀有遗物",
                 "grid_width":1,"grid_height":1,"weight":0.5,"rarity":"rare","description":"",
                 "stack_count":1,"spirit_quality":1.0,"durability":1.0}}]}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route(payload, 0);

        assertFalse(result.isHandled(),
                "world_pos_z 缺失 → entry 非法 → 整份 payload 应被拒(unhandled)，实际 log=" + result.logMessage());
        assertTrue(DroppedItemStore.snapshot().isEmpty(),
                "world_pos_z 缺失 → entry 非法 → store 不应被污染");
    }

    @Test
    void droppedLootSyncRejectsInvalidChargesField() {
        String payload = """
            {"v":1,"type":"dropped_loot_sync","drops":[
              {"instance_id":9100,"source_container_id":"main_pack","source_row":0,"source_col":0,
               "world_pos_x":1.0,"world_pos_y":64.0,"world_pos_z":2.0,
               "item":{"instance_id":9100,"item_id":"ancient_relic","display_name":"上古遗物",
                 "grid_width":1,"grid_height":1,"weight":0.5,"rarity":"ancient","description":"",
                 "stack_count":1,"spirit_quality":1.0,"durability":1.0,"charges":"bad"}}]}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route(payload, 0);

        assertFalse(result.isHandled(),
                "charges 非整数 → item 解析失败 → entry 被拒 → 整份 payload 应 unhandled，实际 log=" + result.logMessage());
        assertTrue(DroppedItemStore.snapshot().isEmpty(),
                "charges 非整数 → item 解析失败 → entry 被拒 → store 不应被污染");
    }

    // ── helpers ──

    /** 过真机生产链：proto DroppedLootSync → ProtoServerDataBridge（生产解码）→ router → DroppedItemStore。 */
    private static void routeProtoWire(Envelope.DroppedLootSync sync) {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setDroppedLootSync(sync)
                .build();

        ProtoServerDataBridge.BridgeResult bridged =
                ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(bridged.isSuccess(),
                "proto→legacyJson 桥接应成功，实际 error=" + bridged.errorMessage());

        String legacyJson = bridged.legacyJson();
        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault()
                .route(legacyJson, legacyJson.getBytes(StandardCharsets.UTF_8).length);

        assertFalse(result.isParseError(),
                "proto-bridged 合法 JSON 不应解析报错(parse error)，实际 log=" + result.logMessage());
        assertTrue(result.isHandled(),
                "dropped_loot_sync 应被 handler 接受（非 noOp）；noOp 说明 entry 解析失败——proto wire 是 flat "
                + "world_pos_x/y/z，handler 读错形状会丢弃整条 entry。log=" + result.logMessage());
        assertEquals("dropped_loot_sync", result.envelope().type());
    }

    private static Envelope.DroppedLootEntry.Builder dropEntry(
            long instanceId, String containerId, double x, double y, double z,
            Envelope.InventoryItemView.Builder item) {
        return Envelope.DroppedLootEntry.newBuilder()
                .setInstanceId(instanceId)
                .setSourceContainerId(containerId)
                .setSourceRow(0)
                .setSourceCol(0)
                .setWorldPosX(x)
                .setWorldPosY(y)
                .setWorldPosZ(z)
                .setItem(item);
    }

    /** 必填字段齐全的 1x1 物品视图骨架（rarity/charges 由调用方按需覆盖）。 */
    private static Envelope.InventoryItemView.Builder baseItemView(long instanceId, String itemId, String name) {
        return Envelope.InventoryItemView.newBuilder()
                .setInstanceId(instanceId)
                .setItemId(itemId)
                .setDisplayName(name)
                .setGridWidth(1)
                .setGridHeight(1)
                .setWeight(0.2)
                .setRarity("common")
                .setStackCount(1)
                .setSpiritQuality(0.5)
                .setDurability(1.0);
    }
}
