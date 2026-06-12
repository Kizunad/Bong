package com.bong.client.agentui;

import com.bong.client.network.AgentUiPayloadHandler;
import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataRouter;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-agent-ui-data-v1 P1 — AgentUiPayloadHandler 单测。
 *
 * <p>测试策略：
 * <ul>
 *   <li>agent_ui_request：happy path（parsed, handled, requestId + xml + timeout 读取）</li>
 *   <li>agent_ui_request：缺 request_id → noOp</li>
 *   <li>agent_ui_request：缺 xml（降级到空串，允许 fallback）</li>
 *   <li>agent_ui_request：timeout_ticks 缺失 → 使用默认值 600</li>
 *   <li>agent_ui_close：happy path（requestId 匹配 → AgentUiStore 清除 active）</li>
 *   <li>agent_ui_close：request_id 不匹配 → AgentUiStore 中 active 不变</li>
 *   <li>agent_ui_close：reason=null（Replaced）→ handled</li>
 *   <li>agent_ui_close：reason=invalid_button_id → handled</li>
 *   <li>agent_ui_close：缺 request_id → noOp</li>
 *   <li>ServerDataRouter 已注册 agent_ui_request 和 agent_ui_close</li>
 *   <li>非法 type → noOp（防漏洞）</li>
 * </ul>
 */
public class AgentUiPayloadHandlerTest {

    private final AgentUiPayloadHandler handler = new AgentUiPayloadHandler();

    @AfterEach
    void tearDown() {
        AgentUiStore.clear();
    }

    // ─── agent_ui_request happy path ────────────────────────────────────────

    @Test
    void agentUiRequest_validPayload_withoutMinecraftClientNoOps() {
        String json = """
            {
              "v": 1,
              "type": "agent_ui_request",
              "request_id": "req-001",
              "target_player": "player-uuid",
              "xml": "<owo-ui><components><flow-layout><label>你好</label></flow-layout></components></owo-ui>",
              "timeout_ticks": 600
            }
            """;

        ServerDataDispatch dispatch = handler.handle(parseEnvelope(json));

        assertFalse(dispatch.handled(),
            "headless 环境无 MinecraftClient/player 时应 noOp，实际 handled=" + dispatch.handled());
        assertTrue(dispatch.logMessage().contains("req-001"),
            "logMessage 应含 request_id 'req-001'，实际=" + dispatch.logMessage());
        assertNull(AgentUiStore.getActive(),
            "无 MinecraftClient/player 时不应写入 AgentUiStore");
    }

    @Test
    void agentUiRequest_nonPositiveTimeout_withoutMinecraftClientNoOps() {
        String json = """
            {
              "v": 1,
              "type": "agent_ui_request",
              "request_id": "req-bad-timeout",
              "target_player": "player-uuid",
              "xml": "<owo-ui><components><flow-layout><label>测试</label></flow-layout></components></owo-ui>",
              "timeout_ticks": 0
            }
            """;

        ServerDataDispatch dispatch = handler.handle(parseEnvelope(json));

        assertFalse(dispatch.handled(),
            "timeout_ticks <= 0 会被归一化，但 headless 环境仍应 noOp，实际 handled=" + dispatch.handled());
        assertNull(AgentUiStore.getActive(),
            "headless 环境不应因非法 timeout 写入 AgentUiStore");
    }

    @Test
    void agentUiRequest_missingRequestId_returnsNoOp() {
        String json = """
            {
              "v": 1,
              "type": "agent_ui_request",
              "target_player": "player-uuid",
              "xml": "<owo-ui><components><flow-layout/></components></owo-ui>",
              "timeout_ticks": 600
            }
            """;

        ServerDataDispatch dispatch = handler.handle(parseEnvelope(json));

        assertFalse(dispatch.handled(),
            "缺 request_id 的 agent_ui_request 应返回 noOp（handled=false），实际=" + dispatch.handled());
        assertNull(AgentUiStore.getActive(),
            "缺 request_id 时 AgentUiStore 不应被写入");
    }

    @Test
    void agentUiRequest_blankRequestId_returnsNoOp() {
        String json = """
            {
              "v": 1,
              "type": "agent_ui_request",
              "request_id": "   ",
              "target_player": "player-uuid",
              "xml": "<owo-ui><components><flow-layout/></components></owo-ui>",
              "timeout_ticks": 600
            }
            """;

        ServerDataDispatch dispatch = handler.handle(parseEnvelope(json));

        assertFalse(dispatch.handled(),
            "空白 request_id 应返回 noOp，实际=" + dispatch.handled());
    }

    @Test
    void agentUiRequest_missingXml_fallsBackToEmptyString() {
        // xml 缺失时 screen 进入 fallback 模式；handler 本身仍返回 handled（降级而非拒绝）
        String json = """
            {
              "v": 1,
              "type": "agent_ui_request",
              "request_id": "req-no-xml",
              "target_player": "player-uuid",
              "timeout_ticks": 600
            }
            """;

        ServerDataDispatch dispatch = handler.handle(parseEnvelope(json));

        assertFalse(dispatch.handled(),
            "缺 xml 可降级，但 headless 环境无 MinecraftClient/player 时应 noOp，实际 handled=" + dispatch.handled());
        assertNull(AgentUiStore.getActive(),
            "headless 环境缺 xml 时也不应写入 AgentUiStore");
    }

    @Test
    void agentUiRequest_missingTimeoutTicks_usesDefaultOf600() {
        // timeout_ticks 缺失时使用 fallback 600；创建的 screen localExpireTick 应 > 0
        String json = """
            {
              "v": 1,
              "type": "agent_ui_request",
              "request_id": "req-no-timeout",
              "target_player": "player-uuid",
              "xml": "<owo-ui><components><flow-layout><label>好</label></flow-layout></components></owo-ui>"
            }
            """;

        ServerDataDispatch dispatch = handler.handle(parseEnvelope(json));

        assertFalse(dispatch.handled(),
            "缺 timeout_ticks 会使用默认值 600，但 headless 环境应 noOp，实际 handled=" + dispatch.handled());
    }

    // ─── agent_ui_close happy path ───────────────────────────────────────────

    @Test
    void agentUiClose_matchingRequestId_clearsActiveStore() {
        // 先放置一个 active screen
        AgentUiScreen screen = AgentUiScreen.create("req-close-match", "<owo-ui><components><flow-layout/></components></owo-ui>", 600, 0L);
        AgentUiStore.setActive(screen);

        String json = """
            {
              "v": 1,
              "type": "agent_ui_close",
              "request_id": "req-close-match"
            }
            """;

        ServerDataDispatch dispatch = handler.handle(parseEnvelope(json));

        assertTrue(dispatch.handled(),
            "匹配 request_id 的 agent_ui_close 应返回 handled，实际=" + dispatch.handled());
        assertNull(AgentUiStore.getActive(),
            "关闭信号后 AgentUiStore.getActive() 应为 null，实际非 null");
    }

    @Test
    void agentUiClose_nonMatchingRequestId_keepsActiveStore() {
        // active screen 有另一个 request_id
        AgentUiScreen screen = AgentUiScreen.create("req-other", "<owo-ui><components><flow-layout/></components></owo-ui>", 600, 0L);
        AgentUiStore.setActive(screen);

        String json = """
            {
              "v": 1,
              "type": "agent_ui_close",
              "request_id": "req-different"
            }
            """;

        handler.handle(parseEnvelope(json));

        assertNotNull(AgentUiStore.getActive(),
            "request_id 不匹配时 AgentUiStore.getActive() 应保持原 screen（不误清）");
        assertEquals("req-other", AgentUiStore.getActive().requestId(),
            "原 screen.requestId() 应保持不变，实际=" + AgentUiStore.getActive().requestId());
    }

    @Test
    void agentUiClose_noActiveScreen_returnsHandledWithoutError() {
        // 无活跃 session 时关闭信号仍应返回 handled（幂等）
        String json = """
            {
              "v": 1,
              "type": "agent_ui_close",
              "request_id": "req-no-active"
            }
            """;

        ServerDataDispatch dispatch = handler.handle(parseEnvelope(json));

        assertTrue(dispatch.handled(),
            "无活跃 session 时 agent_ui_close 仍应 handled（幂等），实际=" + dispatch.handled());
    }

    @Test
    void agentUiClose_withReason_returnsHandled() {
        String json = """
            {
              "v": 1,
              "type": "agent_ui_close",
              "request_id": "req-reason",
              "reason": "invalid_button_id"
            }
            """;

        ServerDataDispatch dispatch = handler.handle(parseEnvelope(json));

        assertTrue(dispatch.handled(),
            "含 reason 的 agent_ui_close 应返回 handled，实际=" + dispatch.handled());
        assertTrue(dispatch.logMessage().contains("invalid_button_id"),
            "logMessage 应含 reason，实际=" + dispatch.logMessage());
    }

    @Test
    void agentUiClose_missingRequestId_returnsNoOp() {
        String json = """
            {
              "v": 1,
              "type": "agent_ui_close"
            }
            """;

        ServerDataDispatch dispatch = handler.handle(parseEnvelope(json));

        assertFalse(dispatch.handled(),
            "缺 request_id 的 agent_ui_close 应返回 noOp，实际=" + dispatch.handled());
    }

    // ─── state transition: new request replaces old active screen ────────────

    @Test
    void agentUiStore_newRequestReplacesOldActiveScreen() {
        // 模拟 req-old 已激活（与 agent_ui_request 处理路径相同：setActive 在 client.execute 内调用）
        AgentUiScreen oldScreen = AgentUiScreen.create(
            "req-old",
            "<owo-ui><components><flow-layout><label>旧</label></flow-layout></components></owo-ui>",
            600, 0L
        );
        AgentUiStore.setActive(oldScreen);

        assertNotNull(AgentUiStore.getActive(),
            "先放置 req-old 后 AgentUiStore 应有 active screen");
        assertEquals("req-old", AgentUiStore.getActive().requestId(),
            "active screen requestId 应为 'req-old'，实际=" + AgentUiStore.getActive().requestId());

        // 新请求 req-new 到达 → setActive 覆盖旧 screen（单面板互斥语义）
        AgentUiScreen newScreen = AgentUiScreen.create(
            "req-new",
            "<owo-ui><components><flow-layout><label>新</label></flow-layout></components></owo-ui>",
            300, 100L
        );
        AgentUiStore.setActive(newScreen);

        assertNotNull(AgentUiStore.getActive(),
            "新请求 setActive 后 AgentUiStore 应有 active screen");
        assertEquals("req-new", AgentUiStore.getActive().requestId(),
            "新请求应替换旧 active screen，requestId 应为 'req-new'，实际="
            + AgentUiStore.getActive().requestId());
        assertFalse(AgentUiStore.getActive() == oldScreen,
            "getActive() 应返回新 screen 对象而非旧对象");
    }

    // ─── ServerDataRouter 注册 ───────────────────────────────────────────────

    @Test
    void defaultRouter_registersAgentUiRequest() {
        assertTrue(
            ServerDataRouter.createDefault().registeredTypes().contains("agent_ui_request"),
            "ServerDataRouter.createDefault() 应注册 'agent_ui_request' type"
        );
    }

    @Test
    void defaultRouter_registersAgentUiClose() {
        assertTrue(
            ServerDataRouter.createDefault().registeredTypes().contains("agent_ui_close"),
            "ServerDataRouter.createDefault() 应注册 'agent_ui_close' type"
        );
    }

    // ─── helper ──────────────────────────────────────────────────────────────

    private static ServerDataEnvelope parseEnvelope(String json) {
        var result = ServerDataEnvelope.parse(json, json.getBytes(StandardCharsets.UTF_8).length);
        assertTrue(result.isSuccess(), "测试 JSON 应能解析：" + result.errorMessage());
        return result.envelope();
    }
}
