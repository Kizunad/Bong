package com.bong.client.network.lingtian;

import bong.Envelope;
import com.bong.client.lingtian.state.LingtianSessionStore;
import com.bong.client.network.ProtoServerDataBridge;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerPayloadParseResult;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-wire-format-bridge-v1 P2/RC3 — {@code lingtian_session.pos} 坐标读取回归。
 *
 * <p>根因：proto {@code LingtianSessionData.pos_x/pos_y/pos_z}（{@code envelope.proto:1353-1355}）
 * 是拆开的 flat int32 三字段，旧 {@code LingtianSessionHandler} 用
 * {@code readIntArray3(p, "pos")} 读**数组**形状的 {@code "pos"} 字段——生产 proto 链下该字段
 * 从不存在，坐标恒静默 fallback {@code {0,0,0}}（比 noOp 更隐蔽：dispatch 仍标 handled，
 * HUD/交互锚点却全部钉在世界原点）。</p>
 *
 * <p>本测试不手写 legacy JSON，而是用生成的 proto builder 真构字节 → 走
 * {@link ProtoServerDataBridge#bridge} 生产解码路径 → {@link ServerDataEnvelope#parse} →
 * {@link LingtianSessionHandler#handle}，断言 {@link LingtianSessionStore} 里的坐标与写入的
 * pos_x/y/z 一致。修复前：readIntArray3 读数组 → 缺失 → fallback {0,0,0} → 本测试 RED
 * （断言 x/y/z 非零且等于写入值）。</p>
 */
public class LingtianSessionHandlerTest {

    @AfterEach
    void tearDown() {
        LingtianSessionStore.replace(LingtianSessionStore.Snapshot.empty());
    }

    @Test
    void sessionSurvivesRealProtoWireFlatCoords() {
        Envelope.LingtianSessionData sessionData = Envelope.LingtianSessionData.newBuilder()
            .setActive(true)
            .setKind(Envelope.LingtianSessionKind.LINGTIAN_SESSION_KIND_PLANTING)
            .setPosX(12)
            .setPosY(64)
            .setPosZ(-7)
            .setElapsedTicks(20)
            .setTargetTicks(160)
            .setPlantId("ci_she_hao")
            .build();

        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
            .setLingtianSession(sessionData)
            .build();

        ProtoServerDataBridge.BridgeResult bridged = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(bridged.isSuccess(),
            "proto→legacyJson 桥接应成功（lingtian_session 走 bridgeStripEnums 路径），实际 error="
            + bridged.errorMessage());

        String legacyJson = bridged.legacyJson();
        ServerPayloadParseResult parsed = ServerDataEnvelope.parse(
            legacyJson, legacyJson.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parsed.isSuccess(),
            "桥输出的 legacyJson 应能被 ServerDataEnvelope 解析，实际 error=" + parsed.errorMessage()
            + "，legacyJson=" + legacyJson);

        ServerDataDispatch dispatch = new LingtianSessionHandler().handle(parsed.envelope());
        assertTrue(dispatch.handled(),
            "lingtian_session 应被 handler 接受（非 noOp），实际 log=" + dispatch.logMessage());

        LingtianSessionStore.Snapshot snapshot = LingtianSessionStore.snapshot();
        assertTrue(snapshot.active(), "active=true 须经 proto wire 存活到 Snapshot.active()");
        assertEquals(LingtianSessionStore.Kind.PLANTING, snapshot.kind(),
            "kind 须剥枚举前缀落地为 PLANTING（P1/RC2 已修，此处顺带回归）");
        assertEquals(12, snapshot.x(),
            "pos_x=12 须经 proto flat 三字段存活到 Snapshot.x()，实际 " + snapshot.x()
            + "——若为 0 说明 handler 仍按数组形状读取已不存在的 \"pos\" 字段，静默 fallback 原点");
        assertEquals(64, snapshot.y(),
            "pos_y=64 须经 proto flat 三字段存活到 Snapshot.y()，实际 " + snapshot.y());
        assertEquals(-7, snapshot.z(),
            "pos_z=-7 须经 proto flat 三字段存活到 Snapshot.z()，实际 " + snapshot.z());
        assertEquals(20, snapshot.elapsedTicks());
        assertEquals(160, snapshot.targetTicks());
        assertEquals("ci_she_hao", snapshot.plantId());
    }

    @Test
    void sessionWithZeroCoordsStillReadsExplicitFlatFields() {
        // 边界：pos_x/y/z 显式为 0（proto3 default，但仍是"真值"而非"缺失"）。
        // includingDefaultValueFields 保证 canonical JSON 仍会输出这三个字段，
        // handler 读到的应是"确实为 0"而非因缺字段才 fallback 0——用非零 elapsed/target
        // 区分“正常解析出 0”与“解析失败 noOp”。
        Envelope.LingtianSessionData sessionData = Envelope.LingtianSessionData.newBuilder()
            .setActive(true)
            .setKind(Envelope.LingtianSessionKind.LINGTIAN_SESSION_KIND_TILL)
            .setPosX(0)
            .setPosY(0)
            .setPosZ(0)
            .setElapsedTicks(5)
            .setTargetTicks(40)
            .build();

        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
            .setLingtianSession(sessionData)
            .build();

        ProtoServerDataBridge.BridgeResult bridged = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(bridged.isSuccess(), "bridge 应成功，实际 error=" + bridged.errorMessage());

        String legacyJson = bridged.legacyJson();
        ServerPayloadParseResult parsed = ServerDataEnvelope.parse(
            legacyJson, legacyJson.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parsed.isSuccess(), "解析应成功，实际 error=" + parsed.errorMessage());

        ServerDataDispatch dispatch = new LingtianSessionHandler().handle(parsed.envelope());
        assertTrue(dispatch.handled(), "非默认坐标(0,0,0)仍应被正常接受，实际 log=" + dispatch.logMessage());

        LingtianSessionStore.Snapshot snapshot = LingtianSessionStore.snapshot();
        assertEquals(0, snapshot.x());
        assertEquals(0, snapshot.y());
        assertEquals(0, snapshot.z());
        assertEquals(5, snapshot.elapsedTicks(),
            "elapsed_ticks=5 须落地，用来区分\"正常解析出坐标 0\"与\"handler 整体 noOp\"");
        assertEquals(40, snapshot.targetTicks());
    }
}
