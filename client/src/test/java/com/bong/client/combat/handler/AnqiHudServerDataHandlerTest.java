package com.bong.client.combat.handler;

import bong.Envelope;
import com.bong.client.hud.AnqiHudState;
import com.bong.client.hud.AnqiHudStateStore;
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

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-combat-skill-feedback-bridges-v1 P4 饱和测试：
 * AnqiHudServerDataHandler + proto 链双端对拍 + ServerDataRouter 路由 + e2e。
 */
class AnqiHudServerDataHandlerTest {

    private AnqiHudServerDataHandler handler;

    @BeforeEach
    void setUp() {
        handler = new AnqiHudServerDataHandler();
        AnqiHudStateStore.clear();
    }

    @AfterEach
    void tearDown() {
        AnqiHudStateStore.clear();
    }

    // ─── kind="echo" → AnqiHudStateStore.echoCount ──────────────

    @Test
    void echoKindWritesEchoCountToStore() {
        ServerDataDispatch dispatch = handler.handle(parse(
            "{\"v\":1,\"type\":\"anqi_hud\",\"kind\":\"echo\",\"echo_count\":3}"
        ));
        assertTrue(dispatch.handled(),
            "handler 应处理 echo payload；实际=" + dispatch.logMessage());
        AnqiHudState state = AnqiHudStateStore.snapshot();
        assertEquals(3, state.echoCount(),
            "echoCount 必须等于 payload.echo_count=3，不重算；实际=" + state.echoCount());
        assertEquals(0f, state.aimProgress(), 0.001f,
            "echo payload 不应设置 aimProgress；实际=" + state.aimProgress());
    }

    @Test
    void echoCountZeroIsAccepted() {
        handler.handle(parse(
            "{\"v\":1,\"type\":\"anqi_hud\",\"kind\":\"echo\",\"echo_count\":0}"
        ));
        assertEquals(0, AnqiHudStateStore.snapshot().echoCount(),
            "echo_count=0 应被接受；实际=" + AnqiHudStateStore.snapshot().echoCount());
    }

    // ─── kind="aim" 是 server 未发送的预留 kind，handler 应 no-op ──

    @Test
    void aimKindIsNoOpBecauseServerDoesNotEmitAim() {
        // anqi_v2 无 aim 前摇事件；server 不发 kind="aim"；handler 应 no-op 不修改 store
        ServerDataDispatch dispatch = handler.handle(parse(
            "{\"v\":1,\"type\":\"anqi_hud\",\"kind\":\"aim\",\"aim_progress\":0.73}"
        ));
        assertFalse(dispatch.handled(),
            "kind='aim' server 当前不发送，handler 应 no-op（延后至 aim-phase plan）；dispatch=" + dispatch.logMessage());
        assertEquals(AnqiHudState.empty(), AnqiHudStateStore.snapshot(),
            "kind='aim' 不应修改 store；实际=" + AnqiHudStateStore.snapshot());
    }

    // ─── kind="charge" → chargeProgress ─────────────────────────

    @Test
    void chargeKindWritesChargeProgressToStore() {
        handler.handle(parse(
            "{\"v\":1,\"type\":\"anqi_hud\",\"kind\":\"charge\",\"charge_progress\":0.5}"
        ));
        AnqiHudState state = AnqiHudStateStore.snapshot();
        assertEquals(0.5f, state.chargeProgress(), 0.001f,
            "chargeProgress 必须等于 0.5；实际=" + state.chargeProgress());
        assertEquals(0f, state.aimProgress(), 0.001f,
            "charge payload 不应设置 aimProgress；实际=" + state.aimProgress());
    }

    // ─── kind="abrasion" → abrasionContainer + abrasionQiPayload ─

    @Test
    void abrasionKindWritesContainerAndQiToStore() {
        handler.handle(parse(
            "{\"v\":1,\"type\":\"anqi_hud\",\"kind\":\"abrasion\","
            + "\"abrasion_container\":\"hand_slot\",\"abrasion_qi_payload\":58.4}"
        ));
        AnqiHudState state = AnqiHudStateStore.snapshot();
        assertTrue(state.hasAbrasionContainer(),
            "abrasion payload 必须设置 abrasionContainer；实际=" + state.abrasionContainer());
        assertEquals("hand_slot", state.abrasionContainer(),
            "abrasionContainer 必须等于 'hand_slot'；实际=" + state.abrasionContainer());
        assertEquals(58.4f, state.abrasionQiPayload(), 0.01f,
            "abrasionQiPayload 必须等于 58.4（after_qi 直接映射，不重算）；实际=" + state.abrasionQiPayload());
    }

    // ─── 未知 kind 静默 no-op ─────────────────────────────────────

    @Test
    void unknownKindIsNoOp() {
        ServerDataDispatch dispatch = handler.handle(parse(
            "{\"v\":1,\"type\":\"anqi_hud\",\"kind\":\"unknown_kind\"}"
        ));
        assertFalse(dispatch.handled(),
            "未知 kind 应 no-op 不修改 store；dispatch=" + dispatch.logMessage());
        assertEquals(AnqiHudState.empty(), AnqiHudStateStore.snapshot(),
            "未知 kind 不应修改 store；实际=" + AnqiHudStateStore.snapshot());
    }

    // ─── 缺 kind 字段 ─────────────────────────────────────────────

    @Test
    void missingKindIsNoOp() {
        ServerDataDispatch dispatch = handler.handle(parse(
            "{\"v\":1,\"type\":\"anqi_hud\",\"echo_count\":5}"
        ));
        assertFalse(dispatch.handled(),
            "缺 kind 字段应 no-op；dispatch=" + dispatch.logMessage());
    }

    // ─── proto 双端对拍：server proto encode → ProtoServerDataBridge ─

    @Test
    void protoAnqiHudEchoRoundtripToLegacyJson() {
        // 锁 proto 链：server 编码 AnqiHud proto → client ProtoServerDataBridge 反序列化
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setAnqiHud(Envelope.AnqiHud.newBuilder()
                        .setKind("echo")
                        .setEchoCount(7)
                        .setAimProgress(0.0)
                        .setChargeProgress(0.0)
                        .setAbrasionContainer("")
                        .setAbrasionQiPayload(0.0)
                        .setTick(42L))
                .build();

        ProtoServerDataBridge.BridgeResult result =
                ProtoServerDataBridge.bridge(envelope.toByteArray());

        assertTrue(result.isSuccess(),
            "proto AnqiHud echo bridge 必须成功；错误=" + result.errorMessage());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals(1, json.get("v").getAsInt(),
            "legacy JSON 版本必须为 1");
        assertEquals("anqi_hud", json.get("type").getAsString(),
            "legacy JSON type 必须为 'anqi_hud'（client 路由键）；实际=" + json.get("type"));
        assertEquals("echo", json.get("kind").getAsString(),
            "legacy JSON kind 必须为 'echo'；实际=" + json.get("kind"));
        assertEquals(7, json.get("echo_count").getAsInt(),
            "echo_count 必须为 7（proto 链不丢字段）；实际=" + json.get("echo_count"));
        assertEquals(42L, json.get("tick").getAsLong(),
            "tick 必须为 42（proto 链不丢字段）；实际=" + json.get("tick"));
    }

    @Test
    void protoAnqiHudAbrasionRoundtripToLegacyJson() {
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setAnqiHud(Envelope.AnqiHud.newBuilder()
                        .setKind("abrasion")
                        .setEchoCount(0)
                        .setAimProgress(0.0)
                        .setChargeProgress(0.0)
                        .setAbrasionContainer("quiver")
                        .setAbrasionQiPayload(23.5)
                        .setTick(100L))
                .build();

        ProtoServerDataBridge.BridgeResult result =
                ProtoServerDataBridge.bridge(envelope.toByteArray());

        assertTrue(result.isSuccess(),
            "proto AnqiHud abrasion bridge 必须成功；错误=" + result.errorMessage());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("anqi_hud", json.get("type").getAsString(),
            "type 必须为 'anqi_hud'");
        assertEquals("abrasion", json.get("kind").getAsString(),
            "kind 必须为 'abrasion'");
        assertEquals("quiver", json.get("abrasion_container").getAsString(),
            "abrasion_container 必须为 'quiver'（proto 链不丢字段）");
        assertEquals(23.5, json.get("abrasion_qi_payload").getAsDouble(), 0.001,
            "abrasion_qi_payload 必须为 23.5（proto 链不丢字段）");
    }

    @Test
    void protoAnqiHudChargeRoundtripToLegacyJson() {
        // server emit QiInjection → kind="charge"，锁 charge_progress 字段 proto 链不丢
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setAnqiHud(Envelope.AnqiHud.newBuilder()
                        .setKind("charge")
                        .setChargeProgress(0.85)
                        .setTick(200L))
                .build();

        ProtoServerDataBridge.BridgeResult result =
                ProtoServerDataBridge.bridge(envelope.toByteArray());

        assertTrue(result.isSuccess(),
            "proto AnqiHud charge bridge 必须成功；错误=" + result.errorMessage());

        JsonObject json = JsonParser.parseString(result.legacyJson()).getAsJsonObject();
        assertEquals("charge", json.get("kind").getAsString(),
            "kind 必须为 'charge'（server QiInjection → charge，非 aim）；实际=" + json.get("kind"));
        assertEquals(0.85, json.get("charge_progress").getAsDouble(), 0.001,
            "charge_progress 必须为 0.85（proto 链不丢字段）；实际=" + json.get("charge_progress"));
    }

    // ─── ServerDataRouter 路由测试 ───────────────────────────────

    @Test
    void routerDispatchesAnqiHudToHandler() {
        // 锁 ServerDataRouter：anqi_hud 路由到 AnqiHudServerDataHandler → store.replace
        ServerDataRouter router = ServerDataRouter.createDefault();

        String json = "{\"v\":1,\"type\":\"anqi_hud\",\"kind\":\"echo\",\"echo_count\":5}";
        ServerDataRouter.RouteResult result = router.route(json, json.getBytes(StandardCharsets.UTF_8).length);

        assertTrue(result.isHandled(),
            "ServerDataRouter 必须将 anqi_hud 路由到 handler；实际=" + result.logMessage());
        assertEquals(5, AnqiHudStateStore.snapshot().echoCount(),
            "路由后 echoCount 必须为 5；实际=" + AnqiHudStateStore.snapshot().echoCount());
    }

    @Test
    void routerRegistersAnqiHudType() {
        // 确认 anqi_hud 类型已注册（无需 handle，只要在 registeredTypes 内即可）
        ServerDataRouter router = ServerDataRouter.createDefault();
        assertTrue(router.registeredTypes().contains("anqi_hud"),
            "'anqi_hud' 必须已在 ServerDataRouter 注册；实际注册类型=" + router.registeredTypes());
    }

    // ─── e2e：proto → bridge → router → store → planner ─────────

    @Test
    void e2eProtoEchoReachesAnqiHudPlanner() {
        // 完整链路：proto bytes → ProtoServerDataBridge → ServerDataRouter → store → planner
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setAnqiHud(Envelope.AnqiHud.newBuilder()
                        .setKind("echo")
                        .setEchoCount(4)
                        .setTick(500L))
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
            "e2e：router 必须 handle anqi_hud；实际=" + routeResult.logMessage());

        // 3. store snapshot
        AnqiHudState state = AnqiHudStateStore.snapshot();
        assertEquals(4, state.echoCount(),
            "e2e：echoCount 必须为 4 (server echo_count=4)；实际=" + state.echoCount());

        // 4. planner produces non-empty commands (echoCount > 0)
        long now = System.currentTimeMillis();
        // State is active if expiresAtMillis > now, which it is since we just set it
        java.util.List<com.bong.client.hud.HudRenderCommand> commands =
                com.bong.client.hud.AnqiHudPlanner.buildCommands(state, now, 800, 600);
        assertFalse(commands.isEmpty(),
            "e2e：echoCount=4 时 AnqiHudPlanner 必须产生非空命令列表（HUD 渲染回路通）；实际=空");
    }

    // ─── e2e：proto charge → bridge → router → store → planner ──

    @Test
    void e2eProtoChargeReachesAnqiHudPlanner() {
        // 完整 charge 链路：proto bytes → ProtoServerDataBridge → ServerDataRouter → store → planner
        // 验证 QiInjection→kind=charge 这条现在有 server producer 的路径端到端通
        Envelope.ServerDataEnvelope envelope = Envelope.ServerDataEnvelope.newBuilder()
                .setAnqiHud(Envelope.AnqiHud.newBuilder()
                        .setKind("charge")
                        .setChargeProgress(0.75f)
                        .setTick(600L))
                .build();

        // 1. proto bridge
        ProtoServerDataBridge.BridgeResult bridgeResult =
                ProtoServerDataBridge.bridge(envelope.toByteArray());
        assertTrue(bridgeResult.isSuccess(),
            "e2e charge：proto bridge 必须成功；错误=" + bridgeResult.errorMessage());

        // 2. router → handler → store
        ServerDataRouter router = ServerDataRouter.createDefault();
        String legacyJson = bridgeResult.legacyJson();
        ServerDataRouter.RouteResult routeResult = router.route(
                legacyJson, legacyJson.getBytes(java.nio.charset.StandardCharsets.UTF_8).length);
        assertTrue(routeResult.isHandled(),
            "e2e charge：router 必须 handle anqi_hud charge；实际=" + routeResult.logMessage());

        // 3. store snapshot
        AnqiHudState state = AnqiHudStateStore.snapshot();
        assertEquals(0.75f, state.chargeProgress(), 0.001f,
            "e2e charge：chargeProgress 必须为 0.75（overload_ratio→charge_progress）；实际=" + state.chargeProgress());
        assertEquals(0f, state.aimProgress(), 0.001f,
            "e2e charge：aimProgress 必须为 0（server 不发 aim）；实际=" + state.aimProgress());

        // 4. planner produces non-empty commands (chargeProgress > 0)
        long now = System.currentTimeMillis();
        java.util.List<com.bong.client.hud.HudRenderCommand> commands =
                com.bong.client.hud.AnqiHudPlanner.buildCommands(state, now, 800, 600);
        assertFalse(commands.isEmpty(),
            "e2e charge：chargeProgress=0.75 时 AnqiHudPlanner 必须产生非空命令列表（charge HUD 渲染回路通）；实际=空");
    }

    // ─── Helper ──────────────────────────────────────────────────

    private static ServerDataEnvelope parse(String json) {
        ServerPayloadParseResult r = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(r.isSuccess(), () -> "parse failed: " + r.errorMessage());
        return r.envelope();
    }
}
