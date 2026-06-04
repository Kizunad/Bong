package com.bong.client.combat.handler;

import bong.Envelope;
import com.bong.client.hud.SwordBondHudState;
import com.bong.client.hud.SwordBondHudStateStore;
import com.bong.client.hud.SwordPathHudPlanner;
import com.bong.client.network.ProtoServerDataBridge;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataRouter;
import com.bong.client.network.ServerPayloadParseResult;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-combat-skill-feedback-bridges-v1 P6 饱和测试：
 * SwordBondHudStateHandler + proto 链双端对拍 + ServerDataRouter 路由 + e2e。
 */
class SwordBondHudStateHandlerTest {

    private SwordBondHudStateHandler handler;

    @BeforeEach
    void setUp() {
        handler = new SwordBondHudStateHandler();
        SwordBondHudStateStore.clear();
    }

    @AfterEach
    void tearDown() {
        SwordBondHudStateStore.clear();
    }

    // ─── active=false：store 重置为 INACTIVE ─────────────────────

    @Test
    void inactivePayloadClearsStore() {
        // 先写入 active 状态
        SwordBondHudStateStore.replace(new SwordBondHudState(true, 4, "固元", 0.5f, 0.8f, false));

        ServerDataDispatch dispatch = handler.handle(parse(
                "{\"v\":1,\"type\":\"sword_bond_hud_state\",\"active\":false,"
                + "\"grade_index\":0,\"grade_name\":\"\","
                + "\"stored_qi_ratio\":0.0,\"bond_strength\":0.0,\"heaven_gate_ready\":false}"
        ));

        assertTrue(dispatch.handled(),
            "active=false payload 应被 handled；实际=" + dispatch.logMessage());
        assertFalse(SwordBondHudStateStore.snapshot().active(),
            "active=false 后 store 应为 INACTIVE；实际=" + SwordBondHudStateStore.snapshot().active());
    }

    // ─── active=true：所有字段写入 store ─────────────────────────

    @Test
    void activePayloadWritesAllFieldsToStore() {
        ServerDataDispatch dispatch = handler.handle(parse(
                "{\"v\":1,\"type\":\"sword_bond_hud_state\",\"active\":true,"
                + "\"grade_index\":4,\"grade_name\":\"固元\","
                + "\"stored_qi_ratio\":0.75,\"bond_strength\":0.9,\"heaven_gate_ready\":false}"
        ));

        assertTrue(dispatch.handled(),
            "active=true payload 应被 handled；实际=" + dispatch.logMessage());

        SwordBondHudState state = SwordBondHudStateStore.snapshot();
        assertTrue(state.active(), "active 应为 true");
        assertEquals(4, state.grade(), "grade_index 必须为 4（固元）；实际=" + state.grade());
        assertEquals("固元", state.gradeName(), "grade_name 必须为 '固元'；实际=" + state.gradeName());
        assertEquals(0.75f, state.storedQiRatio(), 0.001f,
            "storedQiRatio 必须为 0.75（守恒红线：只读展示，不二次扣 qi）；实际=" + state.storedQiRatio());
        assertEquals(0.9f, state.bondStrength(), 0.001f,
            "bondStrength 必须为 0.9；实际=" + state.bondStrength());
        assertFalse(state.heavenGateReady(), "heavenGateReady 应为 false；实际=" + state.heavenGateReady());
    }

    @Test
    void heavenGateReadyTruePropagatedToStore() {
        handler.handle(parse(
                "{\"v\":1,\"type\":\"sword_bond_hud_state\",\"active\":true,"
                + "\"grade_index\":6,\"grade_name\":\"化虚\","
                + "\"stored_qi_ratio\":1.0,\"bond_strength\":1.0,\"heaven_gate_ready\":true}"
        ));

        assertTrue(SwordBondHudStateStore.snapshot().heavenGateReady(),
            "heaven_gate_ready=true 必须正确写入 store；实际=" + SwordBondHudStateStore.snapshot().heavenGateReady());
    }

    // ─── 边界值：ratio/strength clamp ────────────────────────────

    @Test
    void ratioClampedToUnit() {
        handler.handle(parse(
                "{\"v\":1,\"type\":\"sword_bond_hud_state\",\"active\":true,"
                + "\"grade_index\":3,\"grade_name\":\"凝脉\","
                + "\"stored_qi_ratio\":2.0,\"bond_strength\":-0.5,\"heaven_gate_ready\":false}"
        ));

        SwordBondHudState state = SwordBondHudStateStore.snapshot();
        assertTrue(state.storedQiRatio() <= 1.0f,
            "storedQiRatio 应 clamp 到 1.0；实际=" + state.storedQiRatio());
        assertTrue(state.bondStrength() >= 0.0f,
            "bondStrength 应 clamp 到 0.0；实际=" + state.bondStrength());
    }

    // ─── proto 双端对拍：server encode → ProtoServerDataBridge ───

    @Test
    void protoSwordBondHudActiveRoundtripToLegacyJson() {
        // 锁 proto 链：server 编码 SwordBondHud proto → client ProtoServerDataBridge 反序列化
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setSwordBondHudState(Envelope.SwordBondHud.newBuilder()
                        .setActive(true)
                        .setGradeIndex(4)
                        .setGradeName("固元")
                        .setStoredQiRatio(0.75f)
                        .setBondStrength(0.9f)
                        .setHeavenGateReady(false))
                .build();

        ProtoServerDataBridge.BridgeResult result =
                ProtoServerDataBridge.bridge(envelope.toByteArray());

        assertTrue(result.isSuccess(),
            "proto SwordBondHud active bridge 必须成功；错误=" + result.errorMessage());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("sword_bond_hud_state", json.get("type").getAsString(),
            "legacy JSON type 必须为 'sword_bond_hud_state'（client 路由键）；实际=" + json.get("type"));
        assertTrue(json.get("active").getAsBoolean(),
            "active 必须为 true；实际=" + json.get("active"));
        assertEquals(4, json.get("grade_index").getAsInt(),
            "grade_index 必须为 4（proto 链不丢字段）；实际=" + json.get("grade_index"));
        assertEquals("固元", json.get("grade_name").getAsString(),
            "grade_name 必须为 '固元'（proto 链不丢字段）；实际=" + json.get("grade_name"));
        assertEquals(0.75f, json.get("stored_qi_ratio").getAsFloat(), 0.001f,
            "stored_qi_ratio 必须为 0.75（守恒红线：只读展示）；实际=" + json.get("stored_qi_ratio"));
        assertFalse(json.get("heaven_gate_ready").getAsBoolean(),
            "heaven_gate_ready 必须为 false；实际=" + json.get("heaven_gate_ready"));
    }

    @Test
    void protoSwordBondHudHeavenGateRoundtrip() {
        // 化虚满储 heaven_gate_ready=true 路径
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setSwordBondHudState(Envelope.SwordBondHud.newBuilder()
                        .setActive(true)
                        .setGradeIndex(6)
                        .setGradeName("化虚")
                        .setStoredQiRatio(1.0f)
                        .setBondStrength(1.0f)
                        .setHeavenGateReady(true))
                .build();

        ProtoServerDataBridge.BridgeResult result =
                ProtoServerDataBridge.bridge(envelope.toByteArray());

        assertTrue(result.isSuccess(),
            "proto SwordBondHud heaven_gate bridge 必须成功；错误=" + result.errorMessage());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("sword_bond_hud_state", json.get("type").getAsString());
        assertTrue(json.get("heaven_gate_ready").getAsBoolean(),
            "heaven_gate_ready=true 必须通过 proto 链到达 client SwordBondHudStateStore");
    }

    @Test
    void protoSwordBondHudInactiveRoundtrip() {
        // inactive 路径（无 SwordBondComponent）
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setSwordBondHudState(Envelope.SwordBondHud.newBuilder()
                        .setActive(false)
                        .setGradeIndex(0)
                        .setGradeName("")
                        .setStoredQiRatio(0f)
                        .setBondStrength(0f)
                        .setHeavenGateReady(false))
                .build();

        ProtoServerDataBridge.BridgeResult result =
                ProtoServerDataBridge.bridge(envelope.toByteArray());

        assertTrue(result.isSuccess());
        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertFalse(json.get("active").getAsBoolean(),
            "inactive payload active 字段应为 false；实际=" + json.get("active"));
    }

    // ─── ServerDataRouter 路由测试 ──────────────────────────────

    @Test
    void routerRegistersSwordBondHudState() {
        ServerDataRouter router = ServerDataRouter.createDefault();
        assertTrue(router.registeredTypes().contains("sword_bond_hud_state"),
            "'sword_bond_hud_state' 必须已在 ServerDataRouter 注册；实际注册类型=" + router.registeredTypes());
    }

    @Test
    void routerDispatchesSwordBondHudStateToHandler() {
        ServerDataRouter router = ServerDataRouter.createDefault();
        String json = "{\"v\":1,\"type\":\"sword_bond_hud_state\","
                + "\"active\":true,\"grade_index\":3,\"grade_name\":\"凝脉\","
                + "\"stored_qi_ratio\":0.5,\"bond_strength\":0.6,\"heaven_gate_ready\":false}";
        ServerDataRouter.RouteResult result = router.route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isHandled(),
            "ServerDataRouter 必须将 sword_bond_hud_state 路由到 handler；实际=" + result.logMessage());
        assertTrue(SwordBondHudStateStore.snapshot().active(),
            "路由后 store 应为 active=true；实际=" + SwordBondHudStateStore.snapshot().active());
        assertEquals(3, SwordBondHudStateStore.snapshot().grade(),
            "路由后 grade 应为 3；实际=" + SwordBondHudStateStore.snapshot().grade());
    }

    // ─── e2e：proto → bridge → router → store → planner ────────

    @Test
    void e2eProtoActiveReachesSwordPathHudPlanner() {
        // 完整链路：proto bytes → ProtoServerDataBridge → ServerDataRouter → store → planner
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setSwordBondHudState(Envelope.SwordBondHud.newBuilder()
                        .setActive(true)
                        .setGradeIndex(5)
                        .setGradeName("通灵")
                        .setStoredQiRatio(0.8f)
                        .setBondStrength(0.9f)
                        .setHeavenGateReady(false))
                .build();

        // 1. proto bridge
        ProtoServerDataBridge.BridgeResult bridgeResult =
                ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(bridgeResult.isSuccess(),
            "e2e：proto bridge 必须成功；错误=" + bridgeResult.errorMessage());

        // 2. router → handler → store
        ServerDataRouter router = ServerDataRouter.createDefault();
        String legacyJson = bridgeResult.legacyJson();
        ServerDataRouter.RouteResult routeResult = router.route(
                legacyJson, legacyJson.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(routeResult.isHandled(),
            "e2e：router 必须 handle sword_bond_hud_state；实际=" + routeResult.logMessage());

        // 3. store snapshot
        SwordBondHudState state = SwordBondHudStateStore.snapshot();
        assertTrue(state.active(), "e2e：store.active 必须为 true");
        assertEquals(5, state.grade(), "e2e：grade 必须为 5（通灵）；实际=" + state.grade());
        assertEquals(0.8f, state.storedQiRatio(), 0.001f,
            "e2e：storedQiRatio 必须为 0.8（守恒只读）；实际=" + state.storedQiRatio());

        // 4. planner produces non-empty commands (渲染回路真接)
        List<com.bong.client.hud.HudRenderCommand> commands =
                SwordPathHudPlanner.buildCommands(state, System.currentTimeMillis(), 800, 600);
        assertFalse(commands.isEmpty(),
            "e2e：active SwordBondHudState 时 SwordPathHudPlanner 必须产生非空命令列表（渲染回路真接）；实际=空");
    }

    @Test
    void e2eProtoHeavenGateReachesPlanner() {
        // heaven_gate_ready=true 路径：额外 HUD 命令
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setSwordBondHudState(Envelope.SwordBondHud.newBuilder()
                        .setActive(true)
                        .setGradeIndex(6)
                        .setGradeName("化虚")
                        .setStoredQiRatio(1.0f)
                        .setBondStrength(1.0f)
                        .setHeavenGateReady(true))
                .build();

        ProtoServerDataBridge.BridgeResult bridgeResult =
                ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(bridgeResult.isSuccess());

        ServerDataRouter router = ServerDataRouter.createDefault();
        String legacyJson = bridgeResult.legacyJson();
        router.route(legacyJson, legacyJson.getBytes(StandardCharsets.UTF_8).length);

        SwordBondHudState stateYes = SwordBondHudStateStore.snapshot();
        assertTrue(stateYes.heavenGateReady(),
            "e2e：heaven_gate_ready=true 必须到达 SwordBondHudStateStore");

        // 无 heaven_gate 时
        SwordBondHudState stateNo = new SwordBondHudState(true, 6, "化虚", 1.0f, 1.0f, false);
        long now = System.currentTimeMillis();
        List<com.bong.client.hud.HudRenderCommand> cmdsNo =
                SwordPathHudPlanner.buildCommands(stateNo, now, 800, 600);
        List<com.bong.client.hud.HudRenderCommand> cmdsYes =
                SwordPathHudPlanner.buildCommands(stateYes, now, 800, 600);

        assertTrue(cmdsYes.size() > cmdsNo.size(),
            "e2e：heaven_gate_ready=true 应比 false 多产生命令；"
                + "noReady=" + cmdsNo.size() + " vs ready=" + cmdsYes.size());
    }

    // ─── Helper ─────────────────────────────────────────────────

    private static ServerDataEnvelope parse(String json) {
        ServerPayloadParseResult r = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(r.isSuccess(), () -> "parse failed: " + r.errorMessage());
        return r.envelope();
    }
}
