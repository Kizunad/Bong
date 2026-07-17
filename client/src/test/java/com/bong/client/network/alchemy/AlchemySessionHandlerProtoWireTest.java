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

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-bughunt-ac-alchemy-hud-zero-targets-v1 — Rust 生产 protobuf 到 Fabric HUD 的炼丹快照回归。
 *
 * <p>active/finished 用例直接读取 Rust 测试通过真实 {@code build_session_data} 与生产 envelope
 * 编码路径生成的共享字节。链路为：Rust fixture → {@link ProtoServerDataBridge} → legacy
 * envelope parser → 炼丹 handler/store → {@link AlchemyProgressHudPlanner}。Rust 普通测试还会
 * 逐字节校验 fixture 未陈旧，因此两端不会各自硬编码同一消息自证。其余 Java builder 用例只测试
 * 客户端对旧/缺省 wire 的兼容降级，不声称覆盖 Rust producer。</p>
 */
class AlchemySessionHandlerProtoWireTest {
    private static final Path RUST_PROTO_FIXTURE_DIR = Path.of("..", "proto", "fixtures");
    private static final String ACTIVE_SESSION_FIXTURE = "alchemy_session_active_v1.pb";
    private static final String FINISHED_SESSION_FIXTURE = "alchemy_session_finished_v1.pb";

    @BeforeEach
    void setUp() {
        resetStores();
    }

    @AfterEach
    void tearDown() {
        resetStores();
    }

    @Test
    void rustProducedActiveSessionTargetsAndStagesDriveFabricHudPlanner() {
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

        ServerDataDispatch sessionDispatch = dispatchRustProductionSessionFixture(
                ACTIVE_SESSION_FIXTURE);
        assertTrue(sessionDispatch.handled(),
                "Rust 生产的 active alchemy_session proto 必须被 handler 接受，实际 log="
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
        assertEquals("炼制中", snapshot.statusLabel());
        assertEquals(List.of("§7AdjustTemp(0.58)"), snapshot.interventionLog(),
                "Rust builder 生成的 guidance/intervention 必须原样进入 Fabric store");
        assertEquals(3, snapshot.stages().size(),
                "全部有料/空料 stage 必须按声明顺序进入 Fabric store");
        assertEquals(0, snapshot.stages().get(0).atTick());
        assertEquals(0, snapshot.stages().get(0).window());
        assertEquals("ci_she_hao×2 + ling_shui×1", snapshot.stages().get(0).summary());
        assertTrue(snapshot.stages().get(0).completed());
        assertFalse(snapshot.stages().get(0).missed());
        assertEquals(40, snapshot.stages().get(1).atTick());
        assertEquals(6, snapshot.stages().get(1).window());
        assertEquals("dan_sha×3", snapshot.stages().get(1).summary());
        assertFalse(snapshot.stages().get(1).completed());
        assertTrue(snapshot.stages().get(1).missed());
        assertEquals(120, snapshot.stages().get(2).atTick());
        assertEquals(4, snapshot.stages().get(2).window());
        assertEquals("", snapshot.stages().get(2).summary(),
                "required=[] 的 stage 必须保持精确空 summary，不得伪造材料提示");
        assertFalse(snapshot.stages().get(2).completed());
        assertFalse(snapshot.stages().get(2).missed());

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
    void rustProducedFinishedSessionRetainsGuidanceButLeavesActiveHudFlow() {
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

        ServerDataDispatch sessionDispatch = dispatchRustProductionSessionFixture(
                FINISHED_SESSION_FIXTURE
        );
        assertTrue(sessionDispatch.handled(),
                "Rust 生产的 finished alchemy_session proto 必须被 handler 接受，实际 log="
                        + sessionDispatch.logMessage());

        AlchemySessionStore.Snapshot snapshot = AlchemySessionStore.snapshot();
        assertFalse(snapshot.isActive(), "finished snapshot 必须离开活跃 HUD 状态");
        assertEquals("hud_contract_recipe", snapshot.recipeId());
        assertEquals(44, snapshot.elapsedTicks());
        assertEquals(180, snapshot.targetTicks(),
                "finished snapshot 仍须保留权威目标供炉内结束态消费");
        assertEquals(0.58f, snapshot.tempCurrent(), 0.0001f);
        assertEquals(0.62f, snapshot.tempTarget(), 0.0001f);
        assertEquals(0.08f, snapshot.tempBand(), 0.0001f);
        assertEquals(7.25, snapshot.qiInjected(), 0.0001);
        assertEquals(12.5, snapshot.qiTarget(), 0.0001);
        assertEquals("已结束", snapshot.statusLabel());
        assertEquals(List.of("§7AdjustTemp(0.58)"), snapshot.interventionLog());
        assertEquals(3, snapshot.stages().size());
        assertEquals(0, snapshot.stages().get(0).atTick());
        assertEquals(0, snapshot.stages().get(0).window());
        assertTrue(snapshot.stages().get(0).completed());
        assertFalse(snapshot.stages().get(0).missed());
        assertEquals(40, snapshot.stages().get(1).atTick());
        assertEquals(6, snapshot.stages().get(1).window());
        assertFalse(snapshot.stages().get(1).completed());
        assertTrue(snapshot.stages().get(1).missed());
        assertEquals(120, snapshot.stages().get(2).atTick());
        assertEquals(4, snapshot.stages().get(2).window());
        assertEquals("", snapshot.stages().get(2).summary());
        assertFalse(snapshot.stages().get(2).completed());
        assertFalse(snapshot.stages().get(2).missed());

        List<HudRenderCommand> commands =
                AlchemyProgressHudPlanner.buildCommands(320, 180, 2_000L);
        assertTrue(commands.stream().noneMatch(command ->
                        command.layer() == HudRenderLayer.PROCESSING_HUD),
                "finished/inactive snapshot 虽保留 guidance，也不得继续渲染活跃炼丹 HUD");
    }

    @Test
    void legacyActiveSessionWithoutGuidanceDowngradesBeforeStoreAndDoesNotRenderHud() {
        assertTrue(dispatchFurnaceThroughWire(
                Envelope.AlchemyFurnace.newBuilder()
                        .setTier(1)
                        .setIntegrity(1.0)
                        .setIntegrityMax(1.0)
                        .setOwnerName("Azure")
                        .setHasSession(true)
                        .build()
        ).handled());

        ServerDataDispatch dispatch = dispatchJavaConstructedSessionThroughWire(
                Envelope.AlchemySession.newBuilder()
                        .setRecipeId("legacy_recipe")
                        .setActive(true)
                        .setStatusLabel("炼制中")
                        .build()
        );
        assertTrue(dispatch.handled());
        assertTrue(dispatch.logMessage().contains("downgraded incomplete active guidance"),
                "旧 wire 请求 active 却缺 target_ticks 时必须留下兼容降级诊断，实际 log="
                        + dispatch.logMessage());

        AlchemySessionStore.Snapshot snapshot = AlchemySessionStore.snapshot();
        assertEquals("legacy_recipe", snapshot.recipeId(),
                "兼容降级不得丢掉 recipe id 诊断线索");
        assertFalse(snapshot.active(),
                "缺省 guidance 的旧 active wire 必须在 handler 边界归一为 inactive");
        assertFalse(snapshot.isActive(),
                "旧 wire 不得继续进入炉内 UI 或中央 HUD 的 active 分支");
        assertEquals(0, snapshot.targetTicks());
        assertTrue(AlchemyProgressHudPlanner.buildCommands(320, 180, 2_000L).stream()
                        .noneMatch(command -> command.layer() == HudRenderLayer.PROCESSING_HUD),
                "缺省 guidance 不得再渲染“炼制 0%”面板");
    }

    @Test
    void zeroDurationActiveSessionWithOtherGuidanceStillDowngradesWithoutInventingTargets() {
        assertTrue(dispatchFurnaceThroughWire(
                Envelope.AlchemyFurnace.newBuilder()
                        .setTier(1)
                        .setIntegrity(1.0)
                        .setIntegrityMax(1.0)
                        .setOwnerName("Azure")
                        .setHasSession(true)
                        .build()
        ).handled());

        ServerDataDispatch dispatch = dispatchJavaConstructedSessionThroughWire(
                baseSession()
                        .setActive(true)
                        .setTargetTicks(0)
                        .setStatusLabel("炼制中")
                        .build()
        );
        assertTrue(dispatch.handled());

        AlchemySessionStore.Snapshot snapshot = AlchemySessionStore.snapshot();
        assertFalse(snapshot.active(),
                "即使其它字段齐全，零 target_ticks 也不能成为可渲染 active session");
        assertFalse(snapshot.isActive());
        assertEquals(0, snapshot.targetTicks(),
                "兼容层必须 fail closed，不得伪造 duration sentinel");
        assertEquals(0.62f, snapshot.tempTarget(), 0.0001f,
                "降级应保留实际收到的诊断 guidance，而不是清空整份快照");
        assertEquals(12.5, snapshot.qiTarget(), 0.0001);
        assertEquals(3, snapshot.stages().size());
        assertTrue(AlchemyProgressHudPlanner.buildCommands(320, 180, 2_000L).stream()
                        .noneMatch(command -> command.layer() == HudRenderLayer.PROCESSING_HUD),
                "零时长 active payload 不得渲染 0% HUD");
    }

    @Test
    void activeSessionWithoutRecipeDowngradesEvenWhenTargetsArePositive() {
        assertTrue(dispatchFurnaceThroughWire(
                Envelope.AlchemyFurnace.newBuilder()
                        .setTier(1)
                        .setIntegrity(1.0)
                        .setIntegrityMax(1.0)
                        .setOwnerName("Azure")
                        .setHasSession(true)
                        .build()
        ).handled());

        ServerDataDispatch dispatch = dispatchJavaConstructedSessionThroughWire(
                baseSession()
                        .clearRecipeId()
                        .setActive(true)
                        .setStatusLabel("炼制中")
                        .build()
        );
        assertTrue(dispatch.handled());

        AlchemySessionStore.Snapshot snapshot = AlchemySessionStore.snapshot();
        assertEquals("", snapshot.recipeId());
        assertFalse(snapshot.active(),
                "缺 recipe id 的 active payload 必须在 handler 边界 fail closed");
        assertFalse(snapshot.isActive());
        assertEquals(180, snapshot.targetTicks(),
                "缺 recipe 的降级不得破坏其余 wire 字段，便于诊断版本错配");
        assertTrue(AlchemyProgressHudPlanner.buildCommands(320, 180, 2_000L).stream()
                        .noneMatch(command -> command.layer() == HudRenderLayer.PROCESSING_HUD));
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
        ServerDataEnvelope parsed = decodeThroughProductionBridge(envelope.toByteArray());
        return new AlchemyFurnaceHandler().handle(parsed);
    }

    private static ServerDataDispatch dispatchRustProductionSessionFixture(String fileName) {
        ServerDataEnvelope parsed = decodeThroughProductionBridge(
                readRustProductionFixture(fileName));
        assertEquals("alchemy_session", parsed.type(),
                "共享 Rust fixture 必须是完整 ServerDataEnvelope.alchemy_session");
        return new AlchemySessionHandler().handle(parsed);
    }

    private static ServerDataDispatch dispatchJavaConstructedSessionThroughWire(
            Envelope.AlchemySession session
    ) {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setAlchemySession(session)
                .build();
        ServerDataEnvelope parsed = decodeThroughProductionBridge(envelope.toByteArray());
        return new AlchemySessionHandler().handle(parsed);
    }

    private static byte[] readRustProductionFixture(String fileName) {
        Path fixturePath = RUST_PROTO_FIXTURE_DIR.resolve(fileName);
        assertTrue(Files.isRegularFile(fixturePath),
                "Rust 生产 protobuf fixture 必须存在：" + fixturePath.toAbsolutePath());
        try {
            return Files.readAllBytes(fixturePath);
        } catch (IOException error) {
            throw new AssertionError(
                    "读取 Rust 生产 protobuf fixture 失败：" + fixturePath.toAbsolutePath(), error);
        }
    }

    private static ServerDataEnvelope decodeThroughProductionBridge(byte[] envelopeBytes) {
        ProtoServerDataBridge.BridgeResult bridged =
                ProtoServerDataBridge.bridge(envelopeBytes);
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
