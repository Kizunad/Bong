package com.bong.client.network;

import com.bong.client.agentui.AgentUiScreen;
import com.bong.client.agentui.AgentUiStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-agent-ui-data-v1 P1 — 已确认 player 可展示后的 AgentUiPayloadHandler 落库测试。
 */
class AgentUiPayloadHandlerReadyClientTest {
    private static final String VALID_XML =
        "<owo-ui><components><flow-layout><label>天道降示</label></flow-layout></components></owo-ui>";
    private static final long AGENT_UI_TIMEOUT_GRACE_TICKS = 20L;

    private final List<String> sentPayloads = new ArrayList<>();

    @BeforeEach
    void setupBackend() {
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sentPayloads.add(new String(payload, StandardCharsets.UTF_8)));
    }

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
        AgentUiStore.clear();
        sentPayloads.clear();
    }

    @Test
    void openReadyRequest_replacesExistingActiveScreen() {
        AgentUiStore.setActive(AgentUiScreen.create(
            "req-old",
            "<owo-ui><components><flow-layout/></components></owo-ui>",
            600,
            0L
        ));

        ServerDataDispatch dispatch = AgentUiPayloadHandler.openReadyRequestForTests(
            "agent_ui_request",
            "req-new",
            "<owo-ui><components><flow-layout><label>新面板</label></flow-layout></components></owo-ui>",
            300,
            10L,
            AgentUiStore::setActive
        );

        assertTrue(dispatch.handled(),
            "ready client 路径应返回 handled=true，实际=" + dispatch.handled());
        AgentUiScreen active = AgentUiStore.getActive();
        assertNotNull(active,
            "新 request 应替换旧 active screen，实际 active=null");
        assertEquals("req-new", active.requestId(),
            "active requestId 应为 req-new，说明新 request 覆盖旧面板，实际=" + active.requestId());
    }

    @Test
    void handleRawRequest_wire_barePayloadWithReadyClient_opensScreen() {
        String serverWireJson =
            "{\"request_id\":\"req-wire-ready\","
                + "\"target_player\":\"offline:Kiz\","
                + "\"xml\":\"<owo-ui><components><flow-layout><label>天道降示</label></flow-layout></components></owo-ui>\","
                + "\"timeout_ticks\":600}";

        ServerDataDispatch dispatch = AgentUiPayloadHandler.handleRawRequestForReadyClientTests(
            serverWireJson,
            42L,
            AgentUiStore::setActive
        );

        assertTrue(dispatch.handled(),
            "ready-client stub 应跑完整 handleRawRequest happy path，不能只靠 null-client NPE 证明解析");
        assertEquals("agent_ui_request", dispatch.routeType(),
            "dispatch routeType 应保持 agent_ui_request，实际=" + dispatch.routeType());
        AgentUiScreen active = AgentUiStore.getActive();
        assertNotNull(active,
            "ready-client stub opener 应写入 AgentUiStore，证明 parse→open 全链路执行");
        assertEquals("req-wire-ready", active.requestId(),
            "active requestId 应来自 server 裸 wire payload，实际=" + active.requestId());
    }

    @Test
    void handleRawRequest_wire_malformedJson_isUnhandledAndDoesNotOpenScreen() {
        assertIgnoredRawPayload("{not-json", "malformed JSON");
    }

    @Test
    void handleRawRequest_wire_missingRequestId_isUnhandledAndDoesNotOpenScreen() {
        assertIgnoredRawPayload(
            "{\"target_player\":\"offline:Kiz\",\"xml\":\"" + VALID_XML + "\",\"timeout_ticks\":600}",
            "missing request_id"
        );
    }

    @Test
    void handleRawRequest_wire_blankRequestId_isUnhandledAndDoesNotOpenScreen() {
        assertIgnoredRawPayload(
            "{\"request_id\":\"   \",\"target_player\":\"offline:Kiz\",\"xml\":\"" + VALID_XML + "\",\"timeout_ticks\":600}",
            "blank request_id"
        );
    }

    @Test
    void handleRawRequest_wire_zeroTimeout_fallsBackToOneTick() {
        assertNonPositiveTimeoutFallsBackToOneTick(0);
    }

    @Test
    void handleRawRequest_wire_negativeTimeout_fallsBackToOneTick() {
        assertNonPositiveTimeoutFallsBackToOneTick(-5);
    }

    private void assertIgnoredRawPayload(String payload, String caseName) {
        ServerDataDispatch dispatch = AgentUiPayloadHandler.handleRawRequestForReadyClientTests(
            payload,
            42L,
            AgentUiStore::setActive
        );

        assertFalse(dispatch.handled(),
            caseName + " 应返回 handled=false，实际=" + dispatch.handled());
        assertEquals("agent_ui_request", dispatch.routeType(),
            caseName + " routeType 应保持 agent_ui_request，实际=" + dispatch.routeType());
        assertNull(AgentUiStore.getActive(),
            caseName + " 不应写入 AgentUiStore，实际 active=" + AgentUiStore.getActive());
    }

    private void assertNonPositiveTimeoutFallsBackToOneTick(int timeoutTicks) {
        long currentTick = 42L;
        long expectedExpireTick = currentTick + 1L + AGENT_UI_TIMEOUT_GRACE_TICKS;

        AgentUiScreen beforeExpiry = openRawRequestWithTimeout("before-expiry-" + timeoutTicks, timeoutTicks, currentTick);
        beforeExpiry.tickLocalTimeout(expectedExpireTick - 1L);
        beforeExpiry.close();
        assertEquals(1, sentPayloads.size(),
            "timeout_ticks=" + timeoutTicks + " 应在 1 tick fallback 的 expireTick-1 前保持打开，close 应发 1 个 dismissed 包，实际=" + sentPayloads.size());

        AgentUiStore.clear();
        sentPayloads.clear();

        AgentUiScreen atExpiry = openRawRequestWithTimeout("at-expiry-" + timeoutTicks, timeoutTicks, currentTick);
        atExpiry.tickLocalTimeout(expectedExpireTick);
        atExpiry.close();
        assertEquals(0, sentPayloads.size(),
            "timeout_ticks=" + timeoutTicks + " 应回退为 1 tick 并在 expireTick 本地关闭，后续 close 不应发包，实际=" + sentPayloads.size());
    }

    private AgentUiScreen openRawRequestWithTimeout(String requestIdSuffix, int timeoutTicks, long currentTick) {
        String payload =
            "{\"request_id\":\"req-timeout-" + requestIdSuffix + "\","
                + "\"target_player\":\"offline:Kiz\","
                + "\"xml\":\"" + VALID_XML + "\","
                + "\"timeout_ticks\":" + timeoutTicks + "}";

        ServerDataDispatch dispatch = AgentUiPayloadHandler.handleRawRequestForReadyClientTests(
            payload,
            currentTick,
            AgentUiStore::setActive
        );

        assertTrue(dispatch.handled(),
            "timeout_ticks=" + timeoutTicks + " 的 ready-client payload 应打开面板，实际 handled=" + dispatch.handled());
        AgentUiScreen active = AgentUiStore.getActive();
        assertNotNull(active,
            "timeout_ticks=" + timeoutTicks + " 应写入 AgentUiStore，实际 active=null");
        return active;
    }
}
