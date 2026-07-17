package com.bong.client.network.alchemy;

import bong.Envelope;
import com.bong.client.alchemy.state.AlchemyAttemptHistoryStore;
import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import com.bong.client.alchemy.state.AlchemyOutcomeForecastStore;
import com.bong.client.alchemy.state.AlchemySessionStore;
import com.bong.client.hud.AlchemyProgressHudPlanner;
import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudRenderLayer;
import com.bong.client.network.ProtoServerDataBridge;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerPayloadParseResult;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-bughunt-ac-alchemy-hud-zero-targets-v1 — 生产 protobuf 到 Fabric HUD 的炼丹快照回归。
 *
 * <p>本测试沿用历史本地成果 {@code 1d20935f} 的真实 proto bridge，并把断言继续推进到
 * {@link AlchemyProgressHudPlanner}。链路为：生成的 protobuf 消息 →
 * {@link ProtoServerDataBridge} → legacy envelope parser → 炼丹 handler/store → HUD planner。
 * server 若重新发出零目标或空 stages，Rust 侧生产 proto pin 与此处 Fabric 消费 pin 会共同撞红。</p>
 */
class AlchemySessionHandlerProtoWireTest {

    @BeforeEach
    void setUp() {
        resetStores();
    }

    @AfterEach
    void tearDown() {
        resetStores();
    }

    @Test
    void activeSessionTargetsAndStagesSurviveProtoWireAndDriveHudPlanner() {
        ServerDataDispatch furnaceDispatch = dispatchFurnaceThroughWire(
                Envelope.AlchemyFurnace.newBuilder()
                        .setPosX(2)
                        .setPosY(64)
                        .setPosZ(3)
                        .setTier(1)
                        .setIntegrity(1.0)
                        .setIntegrityMax(1.0)
                        .setOwnerName("Azure")
                        .setHasSession(true)
                        .build()
        );
        assertTrue(furnaceDispatch.handled(),
                "active furnace proto 必须进入 AlchemyFurnaceStore，实际 log="
                        + furnaceDispatch.logMessage());

        Envelope.AlchemySession session = baseSession()
                .setActive(true)
                .setStatusLabel("炼制中")
                .build();
        ServerDataDispatch sessionDispatch = dispatchSessionThroughWire(session);
        assertTrue(sessionDispatch.handled(),
                "alchemy_session proto 必须被 handler 接受，实际 log="
                        + sessionDispatch.logMessage());

        AlchemySessionStore.Snapshot snapshot = AlchemySessionStore.snapshot();
        assertTrue(snapshot.isActive(), "active recipe session 必须落地为活跃 Fabric 快照");
        assertEquals("hud_contract_recipe", snapshot.recipeId());
        assertEquals(44, snapshot.elapsedTicks());
        assertEquals(180, snapshot.targetTicks(),
                "target_ticks 经生产 proto bridge 后不得归零，否则 HUD 会显示 0%");
        assertEquals(0.58f, snapshot.tempCurrent(), 0.0001f);
        assertEquals(0.62f, snapshot.tempTarget(), 0.0001f);
        assertEquals(0.08f, snapshot.tempBand(), 0.0001f);
        assertEquals(7.25, snapshot.qiInjected(), 0.0001);
        assertEquals(12.5, snapshot.qiTarget(), 0.0001,
                "qi_target 经生产 proto bridge 后不得归零");
        assertEquals(3, snapshot.stages().size(),
                "全部有料/空料 stage 必须按声明顺序进入 Fabric store");
        assertEquals("ci_she_hao×2 + ling_shui×1", snapshot.stages().get(0).summary());
        assertTrue(snapshot.stages().get(0).completed());
        assertEquals("dan_sha×3", snapshot.stages().get(1).summary());
        assertTrue(snapshot.stages().get(1).missed());
        assertEquals("", snapshot.stages().get(2).summary(),
                "required=[] 的 stage 必须保持精确空 summary，不得伪造材料提示");

        List<HudRenderCommand> commands =
                AlchemyProgressHudPlanner.buildCommands(320, 180, 2_000L);
        assertTrue(commands.stream().anyMatch(command ->
                        command.layer() == HudRenderLayer.PROCESSING_HUD
                                && "炼制 24% · 炼制中".equals(command.text())),
                "Fabric HUD planner 必须用 44/180 真实目标渲染非零进度文案");
        assertTrue(commands.stream().anyMatch(command ->
                        command.layer() == HudRenderLayer.PROCESSING_HUD && command.isRect()),
                "活跃炼丹 session 必须进入真实 HUD 命令流");
    }

    @Test
    void finishedSessionRetainsGuidanceInStoreButLeavesActiveHudFlow() {
        ServerDataDispatch furnaceDispatch = dispatchFurnaceThroughWire(
                Envelope.AlchemyFurnace.newBuilder()
                        .setTier(1)
                        .setIntegrity(1.0)
                        .setIntegrityMax(1.0)
                        .setOwnerName("Azure")
                        .setHasSession(false)
                        .build()
        );
        assertTrue(furnaceDispatch.handled());

        ServerDataDispatch sessionDispatch = dispatchSessionThroughWire(
                baseSession().setActive(false).setStatusLabel("已结束").build()
        );
        assertTrue(sessionDispatch.handled());

        AlchemySessionStore.Snapshot snapshot = AlchemySessionStore.snapshot();
        assertFalse(snapshot.isActive(), "finished snapshot 必须离开活跃 HUD 状态");
        assertEquals(180, snapshot.targetTicks(),
                "finished snapshot 仍须保留权威目标供炉内结束态消费");
        assertEquals(12.5, snapshot.qiTarget(), 0.0001);
        assertEquals(3, snapshot.stages().size());
        assertTrue(snapshot.stages().get(0).completed());
        assertTrue(snapshot.stages().get(1).missed());
        assertEquals("", snapshot.stages().get(2).summary());

        List<HudRenderCommand> commands =
                AlchemyProgressHudPlanner.buildCommands(320, 180, 2_000L);
        assertTrue(commands.stream().noneMatch(command ->
                        command.layer() == HudRenderLayer.PROCESSING_HUD),
                "finished/inactive snapshot 虽保留 guidance，也不得继续渲染活跃炼丹 HUD");
    }

    private static Envelope.AlchemySession.Builder baseSession() {
        return Envelope.AlchemySession.newBuilder()
                .setRecipeId("hud_contract_recipe")
                .setElapsedTicks(44)
                .setTargetTicks(180)
                .setTempCurrent(0.58)
                .setTempTarget(0.62)
                .setTempBand(0.08)
                .setQiInjected(7.25)
                .setQiTarget(12.5)
                .addStages(Envelope.AlchemyStageHint.newBuilder()
                        .setAtTick(0)
                        .setWindow(0)
                        .setSummary("ci_she_hao×2 + ling_shui×1")
                        .setCompleted(true)
                        .setMissed(false))
                .addStages(Envelope.AlchemyStageHint.newBuilder()
                        .setAtTick(40)
                        .setWindow(6)
                        .setSummary("dan_sha×3")
                        .setCompleted(false)
                        .setMissed(true))
                .addStages(Envelope.AlchemyStageHint.newBuilder()
                        .setAtTick(120)
                        .setWindow(4)
                        .setSummary("")
                        .setCompleted(false)
                        .setMissed(false))
                .addInterventionsRecent("§7AdjustTemp(0.58)");
    }

    private static ServerDataDispatch dispatchFurnaceThroughWire(
            Envelope.AlchemyFurnace furnace
    ) {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setAlchemyFurnace(furnace)
                .build();
        ServerDataEnvelope parsed = decodeThroughProductionBridge(envelope);
        return new AlchemyFurnaceHandler().handle(parsed);
    }

    private static ServerDataDispatch dispatchSessionThroughWire(
            Envelope.AlchemySession session
    ) {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setAlchemySession(session)
                .build();
        ServerDataEnvelope parsed = decodeThroughProductionBridge(envelope);
        return new AlchemySessionHandler().handle(parsed);
    }

    private static ServerDataEnvelope decodeThroughProductionBridge(
            Envelope.ServerDataEnvelope envelope
    ) {
        ProtoServerDataBridge.BridgeResult bridged =
                ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(bridged.isSuccess(),
                "proto→legacyJson 桥接应成功，实际 error=" + bridged.errorMessage());

        String legacyJson = bridged.legacyJson();
        ServerPayloadParseResult parsed = ServerDataEnvelope.parse(
                legacyJson, legacyJson.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(parsed.isSuccess(),
                "桥输出的 legacyJson 应能被解析，实际 error=" + parsed.errorMessage()
                        + "，legacyJson=" + legacyJson);
        return parsed.envelope();
    }

    private static void resetStores() {
        AlchemySessionStore.resetForTests();
        AlchemyFurnaceStore.resetForTests();
        AlchemyOutcomeForecastStore.resetForTests();
        AlchemyAttemptHistoryStore.resetForTests();
    }
}
