package com.bong.client.network;

import bong.Envelope;
import com.bong.client.botany.BotanyHarvestMode;
import com.bong.client.botany.HarvestSessionStore;
import com.bong.client.botany.HarvestSessionViewModel;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * fix/proto-worldpos-flat-cluster —— BotanyHarvestProgress.target_pos 真机 proto wire 回归。
 *
 * <p><b>根因</b>：proto {@code BotanyHarvestProgress.target_pos_x/y/z}（envelope.proto:1239-1241）
 * 是 {@code optional double}（Rust {@code Option<[f64;3]>} 拆三字段落地），但
 * {@link ProtoServerDataBridge} 用 {@code JsonFormat.preservingProtoFieldNames()} 桥接 proto→legacy
 * JSON 时**不做 flat→array 重塑**（见该类类注释）。旧 handler 用
 * {@code readOptionalDoubleTriple(payload,"target_pos")} 读**数组**形状的 "target_pos" 键，但生产走
 * protobuf 时 JSON 里只有 "target_pos_x"/"target_pos_y"/"target_pos_z" 三个 flat 键，永远读不到 →
 * target_pos 静默丢失（采集会话仍应用，只是没有目标坐标，UI 无法高亮/寻路）。
 *
 * <p>{@link #botanyHarvestProgressWithTargetPosSurvivesRealProtoWire()} 走真机生产链（proto builder
 * 真构字节 → {@link ProtoServerDataBridge#bridge} → {@link ServerDataEnvelope#parse} → handler →
 * {@link HarvestSessionStore}），修复前 target_pos 应为 null 但断言非 null → RED，恰好锁死本次修复。
 *
 * <p>{@link #botanyHarvestProgressWithoutTargetPosDegradesGracefully()} 反向锁住 optional 降级语义：
 * target_pos_x/y/z 三个字段**都不 set**（proto 显式 presence，未 set 则不会出现在桥接后的 JSON 里）时，
 * 会话仍应被接受（不 brick），target_pos 保持 null——这是合法的「无目标」，不是错误。
 */
public class BotanyHarvestProgressHandlerTest {
    @AfterEach
    void tearDown() {
        HarvestSessionStore.resetForTests();
    }

    /** 一个最小但完整（不会被 handler noOp）的 BotanyHarvestProgress proto builder。 */
    private static Envelope.BotanyHarvestProgress.Builder baseProgress(String sessionId) {
        return Envelope.BotanyHarvestProgress.newBuilder()
            .setSessionId(sessionId)
            .setTargetId("plant-proto-1")
            .setTargetName("寒潭夜合")
            .setPlantKind("han_tan_ye_he")
            .setMode("manual")
            .setProgress(0.42)
            .setAutoSelectable(true)
            .setRequestPending(false)
            .setInterrupted(false)
            .setCompleted(false)
            .setDetail("露重风寒，慎防打滑")
            .addHazardHints("靠近后真元每秒缓慢流失");
    }

    /** 把 BotanyHarvestProgress proto 过真机生产链（proto bytes → bridge → parse → handler），返回 dispatch。 */
    private static ServerDataDispatch handleThroughProtoWire(Envelope.BotanyHarvestProgress progress) {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
            .setBotanyHarvestProgress(progress)
            .build();

        ProtoServerDataBridge.BridgeResult bridged = ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(bridged.isSuccess(),
            "proto→legacyJson 桥接应成功，实际 error=" + bridged.errorMessage());

        String legacyJson = bridged.legacyJson();
        ServerPayloadParseResult parsed = ServerDataEnvelope.parse(
            legacyJson, legacyJson.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parsed.isSuccess(),
            "桥输出的 legacyJson 应能被解析，实际 error=" + parsed.errorMessage());

        return new BotanyHarvestProgressHandler().handle(parsed.envelope());
    }

    @Test
    void botanyHarvestProgressWithTargetPosSurvivesRealProtoWire() {
        Envelope.BotanyHarvestProgress progress = baseProgress("session-botany-proto-pos")
            .setTargetPosX(101.5)
            .setTargetPosY(64.0)
            .setTargetPosZ(-32.25)
            .build();

        ServerDataDispatch dispatch = handleThroughProtoWire(progress);
        assertTrue(dispatch.handled(),
            "target_pos_x/y/z 均已设置的采集会话应被接受，实际 log=" + dispatch.logMessage());

        HarvestSessionViewModel snapshot = HarvestSessionStore.snapshot();
        assertEquals("session-botany-proto-pos", snapshot.sessionId(),
            "store 应反映刚经 proto wire 应用的会话");
        assertTrue(snapshot.hasTargetPos(),
            "proto target_pos_x/y/z 三个 optional 字段都已 set，桥接后 legacyJson 应含"
            + " target_pos_x/y/z 三个 flat 键，handler 须读出并组成坐标；实际 targetPos="
            + java.util.Arrays.toString(snapshot.targetPos())
            + "——若为 null，说明 handler 仍在按旧数组形状 \"target_pos\" 读，取不到 flat 字段（回归复发）");
        assertArrayEquals(new double[] {101.5, 64.0, -32.25}, snapshot.targetPos(), 0.0001,
            "target_pos 坐标须与 proto 里 setTargetPosX/Y/Z 的值一致，实际="
            + java.util.Arrays.toString(snapshot.targetPos()));
    }

    @Test
    void botanyHarvestProgressWithoutTargetPosDegradesGracefully() {
        // 故意不 set target_pos_x/y/z 三个 optional 字段：proto 显式 presence 语义下，
        // 桥接后的 legacyJson 里根本不会出现这三个键（见 ProtoServerDataBridge 用
        // includingDefaultValueFields() 但对显式 optional 未 set 字段不强制输出）。
        Envelope.BotanyHarvestProgress progress = baseProgress("session-botany-proto-nopos").build();

        ServerDataDispatch dispatch = handleThroughProtoWire(progress);
        assertTrue(dispatch.handled(),
            "无 target_pos 是合法的「无目标」降级，采集会话仍应被接受（不 brick），实际 log="
            + dispatch.logMessage());

        HarvestSessionViewModel snapshot = HarvestSessionStore.snapshot();
        assertEquals("session-botany-proto-nopos", snapshot.sessionId(),
            "会话其余字段（session_id 等）应正常落地，不受 target_pos 缺失影响");
        assertFalse(snapshot.hasTargetPos(),
            "target_pos_x/y/z 均未 set，hasTargetPos() 应为 false（合法降级，不是错误），实际 targetPos="
            + java.util.Arrays.toString(snapshot.targetPos()));
        assertNull(snapshot.targetPos(),
            "target_pos 缺失时应保持 null 而不是拼出半个/零值坐标，实际="
            + java.util.Arrays.toString(snapshot.targetPos()));
    }

    @Test
    void validPayloadUpdatesHarvestSessionStore() throws IOException {
        String json = PayloadFixtureLoader.readText("valid-botany-harvest-progress.json");
        ServerDataDispatch dispatch = new BotanyHarvestProgressHandler().handle(parseEnvelope(json));

        assertTrue(dispatch.handled());
        HarvestSessionViewModel snapshot = HarvestSessionStore.snapshot();
        assertEquals("session-botany-01", snapshot.sessionId());
        assertEquals(BotanyHarvestMode.MANUAL, snapshot.mode());
        assertEquals(0.6, snapshot.progress(), 0.0001);
        assertEquals(List.of("靠近后真元每秒缓慢流失"), snapshot.hazardHints());
    }

    @Test
    void missingSessionIdBecomesSafeNoOp() {
        ServerDataDispatch dispatch = new BotanyHarvestProgressHandler().handle(parseEnvelope(
            "{\"v\":1,\"type\":\"botany_harvest_progress\",\"target_name\":\"开脉草\"}"
        ));

        assertFalse(dispatch.handled());
        assertTrue(HarvestSessionStore.snapshot().isEmpty());
        assertTrue(dispatch.logMessage().contains("session_id"));
    }

    private static ServerDataEnvelope parseEnvelope(String json) {
        ServerPayloadParseResult parseResult = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parseResult.isSuccess(), parseResult.errorMessage());
        return parseResult.envelope();
    }
}
