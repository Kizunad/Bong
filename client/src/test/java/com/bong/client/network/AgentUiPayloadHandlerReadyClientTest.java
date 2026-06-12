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
}
