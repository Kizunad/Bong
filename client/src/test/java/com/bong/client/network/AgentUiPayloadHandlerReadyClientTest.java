package com.bong.client.network;

import com.bong.client.agentui.AgentUiScreen;
import com.bong.client.agentui.AgentUiStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-agent-ui-data-v1 P1 — 已确认 player 可展示后的 AgentUiPayloadHandler 落库测试。
 */
class AgentUiPayloadHandlerReadyClientTest {

    @AfterEach
    void tearDown() {
        AgentUiStore.clear();
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
}
