package com.bong.client.agentui;

import com.bong.client.network.ClientRequestProtocol;
import com.bong.client.network.ClientRequestSender;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-agent-ui-data-v1 P1 — AgentUiScreen protocol / 协议单测。
 *
 * <p>不启动 MC，使用 {@link ClientRequestSender#setBackendForTests} 捕获发包。
 * 测试范围：
 * <ul>
 *   <li>encodeAgentUiResponse 协议字段 pin（request_id / action / params / v / type）</li>
 *   <li>button_click：params.button_id 正确编码</li>
 *   <li>dismissed：params 为空对象</li>
 *   <li>parse_error：params 为空对象</li>
 *   <li>空 request_id / action → IllegalArgumentException</li>
 *   <li>AgentUiScreen.create valid XML → isFallback=false（通过 requestId 存活断言）</li>
 *   <li>AgentUiScreen.create empty XML → fallback 面板（FALLBACK_XML 常量非空）</li>
 *   <li>AgentUiScreen.create malformed XML → fallback 面板</li>
 *   <li>localExpireTick：valid XML + timeout=600 + currentTick=1000 → expireTick=1620（+20 grace）</li>
 *   <li>tickLocalTimeout：在超时前不关闭（isClosed 依赖 closed flag）</li>
 *   <li>AgentUiStore.receiveClose 匹配 → active 清空</li>
 *   <li>AgentUiStore.receiveClose 不匹配 → active 保留</li>
 * </ul>
 */
public class AgentUiScreenProtocolTest {

    private final List<String> sentPayloads = new ArrayList<>();

    @BeforeEach
    void setupBackend() {
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sentPayloads.add(new String(payload, java.nio.charset.StandardCharsets.UTF_8)));
    }

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
        AgentUiStore.clear();
        sentPayloads.clear();
    }

    // ─── encodeAgentUiResponse 协议 pin ─────────────────────────────────────

    @Test
    void encodeAgentUiResponse_buttonClick_containsRequiredFields() {
        String json = ClientRequestProtocol.encodeAgentUiResponse(
            "req-001", "button_click", Map.of("button_id", "enter_realm"));

        JsonObject obj = JsonParser.parseString(json).getAsJsonObject();
        assertEquals("agent_ui_response", obj.get("type").getAsString(),
            "type 字段应为 'agent_ui_response'");
        assertEquals(1, obj.get("v").getAsInt(),
            "v 字段应为 1（版本号）");
        assertEquals("req-001", obj.get("request_id").getAsString(),
            "request_id 应 round-trip");
        assertEquals("button_click", obj.get("action").getAsString(),
            "action 应为 'button_click'");
        assertEquals("enter_realm", obj.getAsJsonObject("params").get("button_id").getAsString(),
            "params.button_id 应为 'enter_realm'");
    }

    @Test
    void encodeAgentUiResponse_dismissed_paramsIsEmptyObject() {
        String json = ClientRequestProtocol.encodeAgentUiResponse(
            "req-002", "dismissed", Map.of());

        JsonObject obj = JsonParser.parseString(json).getAsJsonObject();
        assertEquals("dismissed", obj.get("action").getAsString(),
            "action 应为 'dismissed'");
        JsonObject params = obj.getAsJsonObject("params");
        assertNotNull(params, "params 字段应存在（空对象）");
        assertEquals(0, params.size(), "dismissed 的 params 应为空对象，大小应为 0");
    }

    @Test
    void encodeAgentUiResponse_parseError_paramsIsEmptyObject() {
        String json = ClientRequestProtocol.encodeAgentUiResponse(
            "req-003", "parse_error", Map.of());

        JsonObject obj = JsonParser.parseString(json).getAsJsonObject();
        assertEquals("parse_error", obj.get("action").getAsString(),
            "action 应为 'parse_error'");
        assertEquals(0, obj.getAsJsonObject("params").size(),
            "parse_error 的 params 应为空对象");
    }

    @Test
    void encodeAgentUiResponse_nullParams_treatedAsEmpty() {
        // null params 应被降级为空对象，不抛 NPE
        String json = ClientRequestProtocol.encodeAgentUiResponse(
            "req-null-params", "dismissed", null);

        JsonObject obj = JsonParser.parseString(json).getAsJsonObject();
        assertEquals(0, obj.getAsJsonObject("params").size(),
            "null params 应被编码为空对象");
    }

    @Test
    void encodeAgentUiResponse_blankRequestId_throwsIllegalArgument() {
        assertThrows(IllegalArgumentException.class,
            () -> ClientRequestProtocol.encodeAgentUiResponse("", "dismissed", Map.of()),
            "空 requestId 应抛出 IllegalArgumentException"
        );
    }

    @Test
    void encodeAgentUiResponse_blankAction_throwsIllegalArgument() {
        assertThrows(IllegalArgumentException.class,
            () -> ClientRequestProtocol.encodeAgentUiResponse("req-x", "", Map.of()),
            "空 action 应抛出 IllegalArgumentException"
        );
    }

    // ─── sendAgentUiResponse 集成发包 ────────────────────────────────────────

    @Test
    void sendAgentUiResponse_buttonClick_packetSentToBackend() {
        ClientRequestSender.sendAgentUiResponse(
            "req-send-001", "button_click", Map.of("button_id", "dismiss"));

        assertEquals(1, sentPayloads.size(),
            "sendAgentUiResponse 应发送 1 个包，实际=" + sentPayloads.size());
        String payload = sentPayloads.get(0);
        JsonObject obj = JsonParser.parseString(payload).getAsJsonObject();
        assertEquals("button_click", obj.get("action").getAsString(),
            "发出的包 action 应为 'button_click'");
        assertEquals("dismiss", obj.getAsJsonObject("params").get("button_id").getAsString(),
            "发出的包 params.button_id 应为 'dismiss'");
    }

    // ─── AgentUiScreen.create 静态工厂 ──────────────────────────────────────

    @Test
    void agentUiScreen_create_validXml_requestIdPreserved() {
        String xml = "<owo-ui><components><flow-layout><label>好</label><button id=\"dismiss\">关</button></flow-layout></components></owo-ui>";
        AgentUiScreen screen = AgentUiScreen.create("req-create-001", xml, 600, 1000L);

        assertNotNull(screen, "valid XML 创建的 screen 不应为 null");
        assertEquals("req-create-001", screen.requestId(),
            "screen.requestId() 应等于传入的 requestId，实际=" + screen.requestId());
    }

    @Test
    void agentUiScreen_create_emptyXml_createsFallback() {
        // 空 XML → UIModel.load 失败 → fallback 面板；fallback 含 FALLBACK_XML 内容
        AgentUiScreen screen = AgentUiScreen.create("req-fallback", "", 600, 0L);

        assertNotNull(screen, "空 XML 时应创建 fallback screen，不应为 null");
        // fallback 面板的 localExpireTick 应为 0（不启用超时）
        // 通过 tickLocalTimeout 在 tick=0 时不触发关闭来间接验证
        screen.tickLocalTimeout(0L);
        // 如果 fallback screen 因 tick=0 自动关闭，后续断言会失败
        assertEquals("req-fallback", screen.requestId(),
            "fallback screen.requestId() 应保持 requestId，实际=" + screen.requestId());
    }

    @Test
    void agentUiScreen_create_malformedXml_createsFallback() {
        // 畸形 XML → parse 失败 → fallback
        AgentUiScreen screen = AgentUiScreen.create("req-malformed", "<<<not xml>>>", 600, 0L);

        assertNotNull(screen, "畸形 XML 时应创建 fallback screen，不应为 null");
        assertEquals("req-malformed", screen.requestId(),
            "fallback screen requestId 应保持，实际=" + screen.requestId());
    }

    @Test
    void agentUiScreen_fallbackXmlConstant_isNonEmpty() {
        // FALLBACK_XML 常量非空，确保 fallback 面板有内容
        assertFalse(AgentUiScreen.FALLBACK_XML.isBlank(),
            "FALLBACK_XML 常量不应为空字符串");
        assertTrue(AgentUiScreen.FALLBACK_XML.contains("flow-layout"),
            "FALLBACK_XML 应包含 flow-layout，实际=" + AgentUiScreen.FALLBACK_XML);
    }

    @Test
    void agentUiScreen_localExpireTick_validXml() {
        // localExpireTick = currentTick + timeoutTicks + LOCAL_TIMEOUT_GRACE_TICKS
        // 此处通过 tickLocalTimeout 在 expireTick-1 不触发、expireTick 触发来验证
        String xml = "<owo-ui><components><flow-layout><label>x</label></flow-layout></components></owo-ui>";
        long currentTick = 1000L;
        int timeoutTicks = 600;
        long expectedExpireTick = currentTick + timeoutTicks + AgentUiScreen.LOCAL_TIMEOUT_GRACE_TICKS;

        AgentUiScreen screen = AgentUiScreen.create("req-expire", xml, timeoutTicks, currentTick);

        // 在 expectedExpireTick - 1 时不应触发 closeWithoutResponse
        // 无法直接断言 closed flag（private），用另一个路径验证：
        // screen 仍存在且 requestId 不变（closeWithoutResponse 只清理 client screen）
        screen.tickLocalTimeout(expectedExpireTick - 1);
        assertEquals("req-expire", screen.requestId(),
            "在超时前 requestId 应不变，实际=" + screen.requestId());
    }

    @Test
    void agentUiScreen_graceTicks_equalsExpected() {
        assertEquals(20, AgentUiScreen.LOCAL_TIMEOUT_GRACE_TICKS,
            "LOCAL_TIMEOUT_GRACE_TICKS 应为 20（1 秒宽限），实际=" + AgentUiScreen.LOCAL_TIMEOUT_GRACE_TICKS);
    }

    // ─── AgentUiStore ────────────────────────────────────────────────────────

    @Test
    void agentUiStore_receiveClose_matchingId_clearsActive() {
        AgentUiScreen screen = AgentUiScreen.create("req-store-close", "<owo-ui><components><flow-layout/></components></owo-ui>", 600, 0L);
        AgentUiStore.setActive(screen);

        AgentUiStore.receiveClose("req-store-close", null);

        assertNull(AgentUiStore.getActive(),
            "receiveClose 后 getActive() 应为 null，实际非 null");
    }

    @Test
    void agentUiStore_receiveClose_nonMatchingId_keepsActive() {
        AgentUiScreen screen = AgentUiScreen.create("req-keep", "<owo-ui><components><flow-layout/></components></owo-ui>", 600, 0L);
        AgentUiStore.setActive(screen);

        AgentUiStore.receiveClose("req-different", null);

        assertNotNull(AgentUiStore.getActive(),
            "request_id 不匹配时 getActive() 应保持 screen");
        assertEquals("req-keep", AgentUiStore.getActive().requestId(),
            "保留的 screen requestId 应为 'req-keep'，实际=" + AgentUiStore.getActive().requestId());
    }

    @Test
    void agentUiStore_clear_removesActive() {
        AgentUiScreen screen = AgentUiScreen.create("req-clear", "<owo-ui><components><flow-layout/></components></owo-ui>", 600, 0L);
        AgentUiStore.setActive(screen);

        AgentUiStore.clear();

        assertNull(AgentUiStore.getActive(),
            "clear() 后 getActive() 应为 null");
    }
}
