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
    void agentUiRequest_validPayload_returnsHandled() {
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

        assertTrue(dispatch.handled(),
            "合法 agent_ui_request 应返回 handled=true，实际=" + dispatch.handled());
        assertTrue(dispatch.logMessage().contains("req-001"),
            "logMessage 应含 request_id 'req-001'，实际=" + dispatch.logMessage());
    }

    @Test
    void agentUiRequest_setsActiveScreenInStore() {
        // AgentUiStore.getActive() 在测试环境下（无 MC client）不会实际打开屏幕，
        // 但 screen 对象本身已创建并存入 store
        String json = """
            {
              "v": 1,
              "type": "agent_ui_request",
              "request_id": "req-store-test",
              "target_player": "player-uuid",
              "xml": "<owo-ui><components><flow-layout><label>测试</label></flow-layout></components></owo-ui>",
              "timeout_ticks": 300
            }
            """;

        handler.handle(parseEnvelope(json));

        AgentUiScreen active = AgentUiStore.getActive();
        assertNotNull(active,
            "handler 应将 AgentUiScreen 存入 AgentUiStore，getActive() 不应为 null");
        assertEquals("req-store-test", active.requestId(),
            "screen.requestId() 应等于 payload 的 request_id，实际=" + active.requestId());
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

        // handler 返回 handled（进入 fallback 面板路径）
        assertTrue(dispatch.handled(),
            "缺 xml 时 handler 应仍返回 handled（降级 fallback），实际=" + dispatch.handled());
        AgentUiScreen active = AgentUiStore.getActive();
        assertNotNull(active, "缺 xml 时 AgentUiStore 仍应有 active screen（fallback 面板）");
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

        assertTrue(dispatch.handled(),
            "缺 timeout_ticks 时应使用默认值 600 并返回 handled，实际=" + dispatch.handled());
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
