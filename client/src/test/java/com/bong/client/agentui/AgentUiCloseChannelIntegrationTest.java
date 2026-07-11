package com.bong.client.agentui;

import com.bong.client.hud.BongToast;
import com.bong.client.hud.HudRenderCommand;
import com.bong.client.network.AgentUiPayloadHandler;
import com.bong.client.network.ClientRequestSender;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.Arguments;
import org.junit.jupiter.params.provider.MethodSource;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
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
    private static final Path WIRE_FIXTURE = Path.of(
        "..", "agent", "packages", "schema", "samples",
        "agent-ui-close.channel-wire.sample.json"
    );
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
        WireCase wireCase = loadWireCase("replaced");
        AgentUiScreen screen = openScreen(wireCase.requestId());

        dispatch(wireCase.payloadUtf8());

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
        WireCase wireCase = loadWireCase("session_expired");
        openScreen(wireCase.requestId());

        dispatch(wireCase.payloadUtf8());
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
    void sessionExpiredAfterEsc_rawBytesConsumePendingOnceWithoutSecondResponse() {
        WireCase wireCase = loadWireCase("session_expired");
        AgentUiScreen screen = openScreen(wireCase.requestId());
        screen.close();

        assertNull(AgentUiStore.getActive(),
            "ESC 应先本地关屏并保留同 request 的错误 close 等待态");
        assertEquals(1, sentResponses.size(),
            "ESC 应精确产生一条 dismissed C2S response");

        String close = wireCase.payloadUtf8();
        dispatch(close);
        runQueuedTask();

        assertEquals("这次天道面板已过期，请重新尝试", currentToastCommand().text(),
            "ESC 后迟到的 session_expired 应进入真实 BongToast HUD command");
        assertEquals(1, sentResponses.size(),
            "处理迟到 close 后不得追加第二条 C2S response");

        BongToast.resetForTests();
        dispatch(close);
        runQueuedTask();

        assertTrue(BongToast.current(System.currentTimeMillis()).isEmpty(),
            "同一 session_expired 重复到达时等待态已消费，不得重复提示");
        assertEquals(1, sentResponses.size(),
            "重复 close 仍不得追加 C2S response");
    }

    @Test
    void sessionExpiredAfterEsc_oldRequestCannotPolluteCompletedNewLifecycle() {
        WireCase oldClose = loadWireCase("session_expired");
        AgentUiScreen oldScreen = openScreen(oldClose.requestId());
        oldScreen.close();

        String newRequestId = "req-after-esc-new-lifecycle";
        openScreen(newRequestId);
        AgentUiStore.receiveClose(newRequestId, null);

        dispatch(oldClose.payloadUtf8());
        runQueuedTask();

        assertNull(AgentUiStore.getActive(),
            "新 request 生命周期完成后不应恢复旧 ESC screen");
        assertTrue(BongToast.current(System.currentTimeMillis()).isEmpty(),
            "新 request 已开始并完成后，旧 request 的迟到 close 不得污染 HUD");
        assertEquals(1, sentResponses.size(),
            "新生命周期隔离期间只应保留旧 ESC 的 dismissed response");
    }

    @Test
    void invalidButtonAfterLocalClick_consumesMatchingCloseOnceAndKeepsSingleC2sResponse() {
        WireCase wireCase = loadWireCase("invalid_button_id");
        AgentUiScreen screen = openScreen(wireCase.requestId());
        screen.simulateButtonClickForTests("forged-button");
        assertNull(AgentUiStore.getActive(),
            "按钮点击应先本地关屏并登记 pending request");
        assertEquals(1, sentResponses.size(),
            "按钮点击应先产生且只产生一条 agent_ui_response C2S");

        String close = wireCase.payloadUtf8();
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

    @Test
    void sharedWireFixtureChannel_matchesProductionReceiverChannel() {
        JsonObject fixture = loadWireFixture();

        assertEquals(
            AgentUiPayloadHandler.AGENT_UI_CLOSE_CHANNEL.toString(),
            fixture.get("channel").getAsString(),
            "共享 fixture channel 必须与 BongNetworkHandler 使用的生产 receiver 常量一致"
        );
    }

    @ParameterizedTest(name = "{0}")
    @MethodSource("invalidPayloads")
    void invalidRawPayload_isolatedWithoutScreenPendingToastOrResponseSideEffects(
        String caseName,
        String invalidPayload
    ) {
        WireCase validClose = loadWireCase("invalid_button_id");
        AgentUiScreen screen = openScreen(validClose.requestId());

        dispatch(invalidPayload);
        runQueuedTask();

        assertSame(screen, AgentUiStore.getActive(),
            caseName + "：非法 payload 不得关闭当前 screen，实际 active 已变化");
        assertTrue(BongToast.current(System.currentTimeMillis()).isEmpty(),
            caseName + "：非法 payload 不得生成 toast");
        assertTrue(sentResponses.isEmpty(),
            caseName + "：非法 S2C payload 不得回流 agent_ui_response");

        screen.simulateButtonClickForTests("forged-button");
        assertNull(AgentUiStore.getActive(),
            caseName + "：构造 pending 后 screen 应已本地关闭");
        assertEquals(1, sentResponses.size(),
            caseName + "：本地按钮点击应精确产生一条 C2S response");

        dispatch(invalidPayload);
        runQueuedTask();

        assertTrue(BongToast.current(System.currentTimeMillis()).isEmpty(),
            caseName + "：非法 payload 不得消费 pending 或生成 toast");
        assertEquals(1, sentResponses.size(),
            caseName + "：非法 payload 不得追加 C2S response");

        dispatch(validClose.payloadUtf8());
        runQueuedTask();

        assertEquals("天道拒绝了这次操作", currentToastCommand().text(),
            caseName + "：非法 payload 后匹配 close 仍应消费 pending，证明 pending 未被污染");
        assertEquals(1, sentResponses.size(),
            caseName + "：权威 close 不得产生第二条 C2S response");
    }

    private static Stream<Arguments> invalidPayloads() {
        return Stream.of(
            Arguments.of("畸形 JSON", "{not valid json"),
            Arguments.of("缺 request_id", "{\"reason\":\"session_expired\"}"),
            Arguments.of("request_id 类型错误", "{\"request_id\":42,\"reason\":\"session_expired\"}"),
            Arguments.of("reason 类型错误", "{\"request_id\":\"req-wire-invalid\",\"reason\":42}")
        );
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
        Runnable task = clientThreadQueue.remove(0);
        assertDoesNotThrow(task::run,
            "bong:agent_ui_close 调度任务不得向 client thread 逃逸异常");
        assertTrue(clientThreadQueue.isEmpty(),
            "执行 close task 后 client-thread queue 应为空，避免重复调度");
    }

    private static WireCase loadWireCase(String name) {
        JsonObject fixture = loadWireFixture();
        for (var element : fixture.getAsJsonArray("cases")) {
            JsonObject entry = element.getAsJsonObject();
            if (name.equals(entry.get("name").getAsString())) {
                return new WireCase(
                    entry.get("request_id").getAsString(),
                    entry.get("payload_utf8").getAsString()
                );
            }
        }
        throw new AssertionError("共享 agent_ui_close wire fixture 缺少 case=" + name);
    }

    private static JsonObject loadWireFixture() {
        try {
            return JsonParser.parseString(
                Files.readString(WIRE_FIXTURE, StandardCharsets.UTF_8)
            ).getAsJsonObject();
        } catch (IOException | RuntimeException error) {
            throw new AssertionError(
                "无法读取共享 agent_ui_close wire fixture=" + WIRE_FIXTURE.toAbsolutePath(),
                error
            );
        }
    }

    private HudRenderCommand currentToastCommand() {
        return BongToast.buildCommand(
            System.currentTimeMillis(),
            text -> text == null ? 0 : text.length() * 6,
            320
        ).orElseThrow(() -> new AssertionError(
            "错误 close 应生成可由 BongHud 渲染的 toast command，实际 command 为空"));
    }

    private record WireCase(String requestId, String payloadUtf8) {}
}
