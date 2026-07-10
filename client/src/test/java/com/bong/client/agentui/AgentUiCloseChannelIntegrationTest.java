package com.bong.client.agentui;

import com.bong.client.hud.BongToast;
import com.bong.client.hud.HudRenderCommand;
import com.bong.client.network.AgentUiPayloadHandler;
import com.bong.client.network.ClientRequestSender;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * {@code bong:agent_ui_close} 等价集成验收：原始 S2C bytes → client 主线程调度 →
 * payload handler → store/screen → BongToast HUD command。
 */
class AgentUiCloseChannelIntegrationTest {
    private final List<Runnable> clientThreadQueue = new ArrayList<>();
    private final List<String> sentResponses = new ArrayList<>();

    @BeforeEach
    void setUp() {
        ClientRequestSender.setBackendForTests((channel, payload) ->
            sentResponses.add(new String(payload, StandardCharsets.UTF_8)));
    }

    @AfterEach
    void tearDown() {
        ClientRequestSender.resetBackendForTests();
        AgentUiStore.clear();
        BongToast.resetForTests();
        clientThreadQueue.clear();
        sentResponses.clear();
    }

    @Test
    void replacedClose_rawBytesAreScheduledThenCloseSilentlyWithoutResponse() {
        AgentUiScreen screen = openScreen("req-integration-replaced");

        dispatch("{\"request_id\":\"req-integration-replaced\"}");

        assertSame(screen, AgentUiStore.getActive(),
            "network receiver 只应排队主线程任务，任务执行前不能提前修改 UI 状态");
        assertEquals(1, clientThreadQueue.size(),
            "一条 close payload 应精确排入一个 client-thread task");

        runQueuedTask();

        assertNull(AgentUiStore.getActive(),
            "Replaced close 的 client-thread task 执行后应关闭匹配 screen");
        assertTrue(BongToast.current(System.currentTimeMillis()).isEmpty(),
            "Replaced 无 reason，期望 HUD 保持静默，实际产生 toast");
        assertTrue(sentResponses.isEmpty(),
            "server close 已是权威终态，client 不应回流额外 agent_ui_response");
    }

    @Test
    void sessionExpiredClose_rawBytesProduceRenderableHudToastWithoutResponse() {
        openScreen("req-integration-expired");

        dispatch("{\"request_id\":\"req-integration-expired\",\"reason\":\"session_expired\"}");
        runQueuedTask();

        assertNull(AgentUiStore.getActive(),
            "session_expired close 经生产调度入口处理后应关闭匹配 screen");
        HudRenderCommand command = currentToastCommand();
        assertEquals("这次天道面板已过期，请重新尝试", command.text(),
            "session_expired 应进入真实 BongToast HUD command 文案");
        assertTrue(sentResponses.isEmpty(),
            "错误 close 只给玩家反馈，不应产生第二条 C2S response");
    }

    @Test
    void invalidButtonAfterLocalClick_consumesMatchingCloseOnceAndKeepsSingleC2sResponse() {
        AgentUiScreen screen = openScreen("req-integration-invalid");
        screen.simulateButtonClickForTests("forged-button");
        assertNull(AgentUiStore.getActive(),
            "按钮点击应先本地关屏并登记 pending request");
        assertEquals(1, sentResponses.size(),
            "按钮点击应先产生且只产生一条 agent_ui_response C2S");

        String close = "{\"request_id\":\"req-integration-invalid\","
            + "\"reason\":\"invalid_button_id\"}";
        dispatch(close);
        runQueuedTask();

        HudRenderCommand command = currentToastCommand();
        assertEquals("天道拒绝了这次操作", command.text(),
            "invalid_button_id 应进入真实 BongToast HUD command 文案");
        assertEquals(1, sentResponses.size(),
            "处理 server 错误 close 后 C2S 总数仍应为原按钮响应 1 条");

        BongToast.resetForTests();
        dispatch(close);
        runQueuedTask();
        assertFalse(BongToast.buildCommand(
            System.currentTimeMillis(), text -> text == null ? 0 : text.length() * 6, 320
        ).isPresent(), "同一 close 重复到达时 pending 已消费，HUD 不应再次产生 toast command");
    }

    private AgentUiScreen openScreen(String requestId) {
        AgentUiScreen screen = AgentUiScreen.create(
            requestId,
            "<owo-ui><components><flow-layout/></components></owo-ui>",
            600,
            0L
        );
        AgentUiStore.setActive(screen);
        return screen;
    }

    private void dispatch(String json) {
        AgentUiPayloadHandler.dispatchRawClose(
            json.getBytes(StandardCharsets.UTF_8),
            clientThreadQueue::add
        );
    }

    private void runQueuedTask() {
        assertEquals(1, clientThreadQueue.size(),
            "执行前应恰有一个待处理 client-thread task");
        clientThreadQueue.remove(0).run();
        assertTrue(clientThreadQueue.isEmpty(),
            "执行 close task 后 client-thread queue 应为空，避免重复调度");
    }

    private HudRenderCommand currentToastCommand() {
        return BongToast.buildCommand(
            System.currentTimeMillis(),
            text -> text == null ? 0 : text.length() * 6,
            320
        ).orElseThrow(() -> new AssertionError(
            "错误 close 应生成可由 BongHud 渲染的 toast command，实际 command 为空"));
    }
}
