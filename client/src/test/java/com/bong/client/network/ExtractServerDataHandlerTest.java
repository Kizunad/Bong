package com.bong.client.network;

import bong.Envelope;
import com.bong.client.tsy.ExtractStateStore;
import com.bong.client.tsy.RiftPortalView;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

public class ExtractServerDataHandlerTest {
    @AfterEach
    void tearDown() {
        ExtractStateStore.resetForTests();
    }

    @Test
    void portalStateUpdatesStore() {
        ServerDataDispatch dispatch = new ExtractServerDataHandler().handle(parseEnvelope(
            "{\"v\":1,\"type\":\"rift_portal_state\",\"entity_id\":42,\"kind\":\"main_rift\",\"direction\":\"exit\",\"family_id\":\"tsy_lingxu_01\",\"world_pos_x\":1,\"world_pos_y\":2,\"world_pos_z\":3,\"trigger_radius\":1.5,\"current_extract_ticks\":160}"
        ));

        assertTrue(dispatch.handled());
        assertEquals(1, ExtractStateStore.snapshot().portals().size());
        assertEquals(42L, ExtractStateStore.snapshot().portals().get(0).entityId());
        assertEquals(1.5, ExtractStateStore.snapshot().portals().get(0).triggerRadius());
    }

    @Test
    void portalRemovedEvictsStore() {
        ExtractServerDataHandler handler = new ExtractServerDataHandler();
        handler.handle(parseEnvelope(
            "{\"v\":1,\"type\":\"rift_portal_state\",\"entity_id\":42,\"kind\":\"main_rift\",\"direction\":\"exit\",\"family_id\":\"tsy_lingxu_01\",\"world_pos_x\":1,\"world_pos_y\":2,\"world_pos_z\":3,\"trigger_radius\":1.5,\"current_extract_ticks\":160}"
        ));

        ServerDataDispatch dispatch = handler.handle(parseEnvelope(
            "{\"v\":1,\"type\":\"rift_portal_removed\",\"entity_id\":42}"
        ));

        assertTrue(dispatch.handled());
        assertEquals(0, ExtractStateStore.snapshot().portals().size());
    }

    // ── fix/proto-worldpos-flat-cluster：rift_portal_state 经真机 proto 生产链回归 ──
    //
    // 根因：生产 --release 下 server→client 走 protobuf，ProtoServerDataBridge 用
    // JsonFormat.preservingProtoFieldNames() 把 proto 解码成 legacy JSON，proto
    // RiftPortalState.world_pos 是拆成 world_pos_x/_y/_z 三个 flat 字段落地（envelope.proto
    // 2742-2744 行），bridge 不做 flat→array 重塑。旧 ExtractServerDataHandler 用
    // readDoubleTriple 读**数组**形状的 "world_pos" 字段 → 生产链下永远拿 null →
    // noOp（"missing entity_id/world_pos"）→ 裂口 portal 永远不进 ExtractStateStore，
    // HUD 上玩家永远看不到可撤离的裂口。
    //
    // 本测试不手写 legacy JSON，而是用生成的 proto builder 真构字节 → 走
    // ProtoServerDataBridge.bridge（生产解码路径）→ ServerDataEnvelope.parse →
    // ExtractServerDataHandler.handle，断言 ExtractStateStore 里的坐标与写入的
    // world_pos_x/y/z 一致。修复前：readDoubleTriple 读数组 → pos==null → noOp →
    // portal 未进 store → dispatch.handled() 为 false，本测试 RED。
    @Test
    void portalStateSurvivesRealProtoWireFlatCoords() {
        Envelope.RiftPortalState portalState = Envelope.RiftPortalState.newBuilder()
            .setEntityId(77)
            .setKind(Envelope.RiftPortalKind.RIFT_PORTAL_KIND_MAIN_RIFT)
            .setDirection(Envelope.RiftPortalDirection.RIFT_PORTAL_DIRECTION_EXIT)
            .setFamilyId("tsy_lingxu_01")
            .setWorldPosX(12.5)
            .setWorldPosY(64.0)
            .setWorldPosZ(-7.25)
            .setTriggerRadius(2.0)
            .setCurrentExtractTicks(10)
            .build();

        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
            .setRiftPortalState(portalState)
            .build();

        ProtoServerDataBridge.BridgeResult bridged = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(bridged.isSuccess(),
            "proto→legacyJson 桥接应成功（rift_portal_state 走通用 extractInner 路径），实际 error="
            + bridged.errorMessage());

        String legacyJson = bridged.legacyJson();
        ServerPayloadParseResult parsed = ServerDataEnvelope.parse(
            legacyJson, legacyJson.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parsed.isSuccess(),
            "桥输出的 legacyJson 应能被 ServerDataEnvelope 解析，实际 error=" + parsed.errorMessage()
            + "，legacyJson=" + legacyJson);

        ServerDataDispatch dispatch = new ExtractServerDataHandler().handle(parsed.envelope());
        assertTrue(dispatch.handled(),
            "rift_portal_state 应被 handler 接受（非 noOp），因为 world_pos_x/_y/_z 均已在 proto 里写入；"
            + "实际 log=" + dispatch.logMessage()
            + "——若为 noOp 说明 handler 仍按数组形状读取 \"world_pos\"，未识别 proto 生产链的 flat 三字段");

        assertEquals(1, ExtractStateStore.snapshot().portals().size(),
            "portal 应已写入 ExtractStateStore（entity_id=77），实际 size="
            + ExtractStateStore.snapshot().portals().size());

        RiftPortalView portal = ExtractStateStore.snapshot().portals().get(0);
        assertEquals(77L, portal.entityId(),
            "entity_id 须经 proto wire 存活，实际 " + portal.entityId());
        assertEquals(12.5, portal.x(), 1e-9,
            "world_pos_x=12.5 须经 proto wire 存活到 RiftPortalView.x()，实际 " + portal.x());
        assertEquals(64.0, portal.y(), 1e-9,
            "world_pos_y=64.0 须经 proto wire 存活到 RiftPortalView.y()，实际 " + portal.y());
        assertEquals(-7.25, portal.z(), 1e-9,
            "world_pos_z=-7.25 须经 proto wire 存活到 RiftPortalView.z()，实际 " + portal.z());

        // plan-wire-format-bridge-v1 P1/RC2 配套验证（收口 §8.1 #1 要求：direction 枚举前缀
        // 修好后，一并验 rift_portal_state 端到端恢复）——kind/direction 须被
        // ProtoServerDataBridge.bridgeStripEnums 剥成裸小写词，而非恒落 proto 全名
        // "RIFT_PORTAL_KIND_MAIN_RIFT" / "RIFT_PORTAL_DIRECTION_EXIT"（那样 TSY 撤离逻辑的
        // "exit" 字面量比较恒不命中，Y 键撤离交互与裂口种类展示双双失效）。
        assertEquals("main_rift", portal.kind(),
            "kind 须剥成 'main_rift'，实际 " + portal.kind()
            + "——若仍是 proto 全名前缀说明 bridgeStripEnums 未接线 rift_portal_state.kind");
        assertEquals("exit", portal.direction(),
            "direction 须剥成 'exit'（TSY 撤离方向判定），实际 " + portal.direction()
            + "——若仍是 'RIFT_PORTAL_DIRECTION_EXIT' 说明 Y 键撤离交互恒不命中 \"exit\" 比较");
    }

    @Test
    void extractProgressLifecycleUpdatesStore() {
        ExtractServerDataHandler handler = new ExtractServerDataHandler();
        handler.handle(parseEnvelope(
            "{\"v\":1,\"type\":\"extract_started\",\"player_id\":\"offline:Kiz\",\"portal_entity_id\":42,\"portal_kind\":\"main_rift\",\"required_ticks\":160,\"at_tick\":100}"
        ));
        handler.handle(parseEnvelope(
            "{\"v\":1,\"type\":\"extract_progress\",\"player_id\":\"offline:Kiz\",\"portal_entity_id\":42,\"elapsed_ticks\":20,\"required_ticks\":160}"
        ));

        assertTrue(ExtractStateStore.snapshot().extracting());
        assertEquals(20, ExtractStateStore.snapshot().elapsedTicks());

        handler.handle(parseEnvelope(
            "{\"v\":1,\"type\":\"extract_aborted\",\"player_id\":\"offline:Kiz\",\"reason\":\"out_of_range\"}"
        ));
        assertTrue(!ExtractStateStore.snapshot().extracting());
        assertTrue(ExtractStateStore.snapshot().message().contains("无法撤离"));
        assertTrue(ExtractStateStore.snapshot().message().contains("距离过远"));
    }

    @Test
    void portalOccupiedAbortUsesDedicatedRaceOutCopy() {
        new ExtractServerDataHandler().handle(parseEnvelope(
            "{\"v\":1,\"type\":\"extract_aborted\",\"player_id\":\"offline:Kiz\",\"reason\":\"portal_occupied\"}"
        ));

        assertTrue(ExtractStateStore.snapshot().message().contains("无法撤离"));
        assertTrue(ExtractStateStore.snapshot().message().contains("裂口被占，换下一个"));
    }

    @Test
    void collapsePayloadStartsCountdown() {
        new ExtractServerDataHandler().handle(parseEnvelope(
            "{\"v\":1,\"type\":\"tsy_collapse_started_ipc\",\"family_id\":\"tsy_lingxu_01\",\"at_tick\":100,\"remaining_ticks\":600,\"collapse_tear_entity_ids\":[1,2,3]}"
        ));

        assertTrue(ExtractStateStore.snapshot().collapseActive(System.currentTimeMillis()));
        assertEquals("tsy_lingxu_01", ExtractStateStore.snapshot().collapsingFamilyId());
    }

    @Test
    void completedPayloadTriggersWhiteFlash() {
        long now = System.currentTimeMillis();
        new ExtractServerDataHandler().handle(parseEnvelope(
            "{\"v\":1,\"type\":\"extract_completed\",\"player_id\":\"offline:Kiz\",\"portal_kind\":\"main_rift\",\"family_id\":\"tsy_lingxu_01\",\"exit_world_pos\":[8,65,9],\"at_tick\":200}"
        ));

        assertTrue(ExtractStateStore.snapshot().screenFlashActive(now));
        assertEquals(0xCCFFFFFF, ExtractStateStore.snapshot().screenFlashColor());
    }

    @Test
    void collapseCountdownReachingZeroTriggersWhiteFlash() {
        ExtractStateStore.markCollapseStarted("tsy_lingxu_01", 1, 1000L);

        ExtractStateStore.tick(1100L);

        assertTrue(ExtractStateStore.snapshot().screenFlashActive(1100L));
        assertEquals(0xCCFFFFFF, ExtractStateStore.snapshot().screenFlashColor());
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
