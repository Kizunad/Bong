package com.bong.client.network;

import bong.Envelope;
import com.bong.client.inventory.state.RemainsStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-remains-suite P0 — remains_sync 端到端回归（照 {@link DroppedLootSyncHandlerTest}）。
 *
 * <p>happy-path 走真机生产链（真构 proto 字节 → {@link ProtoServerDataBridge} → router →
 * {@link RemainsStore}），锁死 proto flat {@code world_pos_x/y/z} 坐标形状——
 * dropped_loot_sync 曾因 handler 读 {@code world_pos} 数组而在 proto 路径全军覆没
 * （单测吃数组 JSON 假路径掩盖），remains_sync 不允许复发。</p>
 */
public class RemainsSyncHandlerTest {

    @AfterEach
    void tearDown() {
        RemainsStore.resetForTests();
    }

    // ── happy path：真机 proto 生产链 ──

    @Test
    void remainsSyncSurvivesRealProtoWire() {
        Envelope.RemainsSync sync = Envelope.RemainsSync.newBuilder()
                .addRemains(Envelope.RemainsEntry.newBuilder()
                        .setRemainsId("3fa85f64-5717-4562-b3fc-2c963f66afa6")
                        .setWorldPosX(8.5)
                        .setWorldPosY(66.0)
                        .setWorldPosZ(-4.25)
                        .setDimension("minecraft:overworld")
                        .setDisplayName("遗骸")
                        .setItemCount(3)
                        .setBoneCoins(12))
                .build();

        routeProtoWire(sync);

        assertEquals(1, RemainsStore.snapshot().size(),
                "proto wire 的遗骸 entry 必须被解析进 store；size=0 说明 world_pos flat 形状不匹配");
        RemainsStore.Entry entry = RemainsStore.get("3fa85f64-5717-4562-b3fc-2c963f66afa6");
        assertNotNull(entry, "remains_id 应在 store 中（proto wire 解析成功）");
        assertEquals(8.5, entry.worldPosX(), "world_pos_x 须经 proto wire (flat) 存活");
        assertEquals(66.0, entry.worldPosY(), "world_pos_y 须经 proto wire (flat) 存活");
        assertEquals(-4.25, entry.worldPosZ(), "world_pos_z 须经 proto wire (flat) 存活");
        assertEquals("minecraft:overworld", entry.dimension());
        assertEquals("遗骸", entry.displayName());
        assertEquals(3, entry.itemCount());
        assertEquals(12L, entry.boneCoins());
    }

    @Test
    void multipleRemainsSurviveProtoWireWithDistinctPositions() {
        Envelope.RemainsSync sync = Envelope.RemainsSync.newBuilder()
                .addRemains(remainsEntry("uuid-1", 1.0, 64.0, 2.0))
                .addRemains(remainsEntry("uuid-2", -10.5, 70.0, 33.25))
                .build();

        routeProtoWire(sync);

        assertEquals(2, RemainsStore.snapshot().size(), "两具遗骸都须进 store");
        assertEquals(2.0, RemainsStore.get("uuid-1").worldPosZ(), "entry uuid-1 坐标不应串味");
        assertEquals(-10.5, RemainsStore.get("uuid-2").worldPosX(), "entry uuid-2 坐标不应串味");
    }

    // ── 空 sync：清空 store ──

    @Test
    void emptyRemainsSyncClearsExistingStore() {
        RemainsStore.putOrReplace(new RemainsStore.Entry(
                "stale", 8.5, 66.0, 8.5, "minecraft:overworld", "遗骸", 1, 0L));

        routeProtoWire(Envelope.RemainsSync.newBuilder().build());

        assertTrue(RemainsStore.snapshot().isEmpty(),
                "空 remains 的 sync 必须清空 store（遗骸被搬空后世界里应消失）");
    }

    // ── 边界：非法 entry 被拒且不抛（flat wire 形状） ──

    @Test
    void malformedRemainsPositionIsRejectedWithoutThrowing() {
        // flat wire 缺 world_pos_z → entry 非法 → 拒绝整份 payload 且不抛异常。
        String payload = """
            {"v":1,"type":"remains_sync","remains":[
              {"remains_id":"uuid-x","world_pos_x":1.0,"world_pos_y":64.0,
               "dimension":"minecraft:overworld","display_name":"遗骸",
               "item_count":1,"bone_coins":0}]}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route(payload, 0);

        assertFalse(result.isHandled(),
                "world_pos_z 缺失 → entry 非法 → 整份 payload 应被拒(unhandled)，实际 log=" + result.logMessage());
        assertTrue(RemainsStore.snapshot().isEmpty(),
                "world_pos_z 缺失 → entry 非法 → store 不应被污染");
    }

    @Test
    void negativeCountsAreRejectedWithoutTruncation() {
        for (String field : new String[] {"item_count", "bone_coins"}) {
            RemainsStore.resetForTests();
            String payload = """
                {"v":1,"type":"remains_sync","remains":[
                  {"remains_id":"uuid-x","world_pos_x":1.0,"world_pos_y":64.0,"world_pos_z":1.0,
                   "dimension":"minecraft:overworld","display_name":"遗骸",
                   "item_count":1,"bone_coins":0}]}
                """.replace("\"" + field + "\":1", "\"" + field + "\":-1")
                   .replace("\"" + field + "\":0", "\"" + field + "\":-1");

            ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route(payload, 0);

            assertFalse(result.isHandled(),
                    field + " 为负数应被拒绝，避免违反 schema minimum:0；actual log=" + result.logMessage());
            assertTrue(RemainsStore.snapshot().isEmpty(),
                    field + " 为负数时 store 不应被污染");
        }
    }

    @Test
    void extremeNegativeItemCountIsRejectedWithoutIntWraparound() {
        String payload = """
            {"v":1,"type":"remains_sync","remains":[
              {"remains_id":"uuid-x","world_pos_x":1.0,"world_pos_y":64.0,"world_pos_z":1.0,
               "dimension":"minecraft:overworld","display_name":"遗骸",
               "item_count":-9223372036854775808,"bone_coins":0}]}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route(payload, 0);

        assertFalse(result.isHandled(),
                "item_count 下界溢出值应被拒绝，不能 intValue() 截断后进入 store；actual log=" + result.logMessage());
        assertTrue(RemainsStore.snapshot().isEmpty(),
                "item_count 下界溢出时 store 不应被污染");
    }

    @Test
    void missingRemainsArrayIsNoOp() {
        String payload = """
            {"v":1,"type":"remains_sync"}
            """;

        ServerDataRouter.RouteResult result = ServerDataRouter.createDefault().route(payload, 0);

        assertFalse(result.isHandled(),
                "remains 数组缺失应 noOp（不抛异常不清 store），实际 log=" + result.logMessage());
    }

    // ── helpers ──

    /** 过真机生产链：proto RemainsSync → ProtoServerDataBridge（生产解码）→ router → RemainsStore。 */
    private static void routeProtoWire(Envelope.RemainsSync sync) {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setRemainsSync(sync)
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
                "remains_sync 应被 handler 接受（非 noOp）；noOp 说明 entry 解析失败——proto wire 是 flat "
                + "world_pos_x/y/z，handler 读错形状会丢弃整条 entry。log=" + result.logMessage());
        assertEquals("remains_sync", result.envelope().type());
    }

    private static Envelope.RemainsEntry.Builder remainsEntry(String id, double x, double y, double z) {
        return Envelope.RemainsEntry.newBuilder()
                .setRemainsId(id)
                .setWorldPosX(x)
                .setWorldPosY(y)
                .setWorldPosZ(z)
                .setDimension("minecraft:overworld")
                .setDisplayName("遗骸")
                .setItemCount(1)
                .setBoneCoins(0);
    }
}
