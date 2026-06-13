package com.bong.client.agentui;

import com.bong.client.network.AgentUiPayloadHandler;
import com.bong.client.network.ServerDataRouter;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

/**
 * plan-agent-ui-data-v1 P1 — AgentUiPayloadHandler 饱和测试。
 *
 * <p>测试策略：
 * <ul>
 *   <li>覆盖生产入口 {@link AgentUiPayloadHandler#handleRawClose(String)}（headless 直测，无 MinecraftClient 依赖）</li>
 *   <li>覆盖 {@link AgentUiPayloadHandler#handleRawRequest(String, net.minecraft.client.MinecraftClient)}
 *       可无 client 到达的早退出路径（JSON 解析错误 / 缺 request_id / 空白 request_id）</li>
 *   <li>wire 对拍：使用 server {@code serde_json} 真实序列化形态（裸 AgentUiRequestPayloadV1/
 *       AgentUiClosePayloadV1，无 v/type 字段；reason=null 时 Rust skip_serializing 省略）；
 *       ready-client happy path 由 {@code AgentUiPayloadHandlerReadyClientTest} 用 stub opener 覆盖，
 *       不再用 null-client NPE 充当解析成功信号。</li>
 *   <li>防回归 pin：ServerDataRouter 不再注册 agent_ui_request/agent_ui_close</li>
 * </ul>
 */
public class AgentUiPayloadHandlerTest {

    @AfterEach
    void tearDown() {
        AgentUiStore.clear();
    }

    // ─────────────────────────────────────────────────────────────────────────
    // handleRawClose — happy path
    // ─────────────────────────────────────────────────────────────────────────

    @Test
    void handleRawClose_matchingRequestId_clearsActiveStore() {
        // 先放置一个 active screen
        AgentUiScreen screen = AgentUiScreen.create(
            "req-close-match",
            "<owo-ui><components><flow-layout/></components></owo-ui>",
            600, 0L
        );
        AgentUiStore.setActive(screen);

        // server 发来的裸 JSON（无 v/type）
        AgentUiPayloadHandler.handleRawClose(
            "{\"request_id\":\"req-close-match\"}"
        );

        assertNull(AgentUiStore.getActive(),
            "匹配 request_id 的 handleRawClose 后 AgentUiStore.getActive() 应为 null，实际非 null");
    }

    @Test
    void handleRawClose_nonMatchingRequestId_keepsActiveStore() {
        AgentUiScreen screen = AgentUiScreen.create(
            "req-other",
            "<owo-ui><components><flow-layout/></components></owo-ui>",
            600, 0L
        );
        AgentUiStore.setActive(screen);

        AgentUiPayloadHandler.handleRawClose(
            "{\"request_id\":\"req-different\"}"
        );

        assertNotNull(AgentUiStore.getActive(),
            "request_id 不匹配时 AgentUiStore.getActive() 应保持原 screen（不误清）");
        assertEquals("req-other", AgentUiStore.getActive().requestId(),
            "原 screen.requestId() 应保持不变，实际=" + AgentUiStore.getActive().requestId());
    }

    @Test
    void handleRawClose_noActiveScreen_doesNotThrow() {
        // 无活跃 session 时关闭信号应幂等，不抛异常
        assertDoesNotThrow(
            () -> AgentUiPayloadHandler.handleRawClose("{\"request_id\":\"req-no-active\"}"),
            "无活跃 session 时 handleRawClose 不应抛出异常（幂等）"
        );
        assertNull(AgentUiStore.getActive(),
            "无活跃 session 时 handleRawClose 后 store 应仍为 null");
    }

    @Test
    void handleRawClose_withReason_clearsMatchingScreen() {
        AgentUiScreen screen = AgentUiScreen.create(
            "req-reason",
            "<owo-ui><components><flow-layout/></components></owo-ui>",
            600, 0L
        );
        AgentUiStore.setActive(screen);

        // server 发来带 reason 字段（session_expired 等）
        AgentUiPayloadHandler.handleRawClose(
            "{\"request_id\":\"req-reason\",\"reason\":\"invalid_button_id\"}"
        );

        assertNull(AgentUiStore.getActive(),
            "含 reason 的 handleRawClose 匹配 request_id 后 store 应已清空，实际非 null");
    }

    @Test
    void handleRawClose_reasonNull_explicitNullValue_clearsMatchingScreen() {
        // reason 字段明确为 JSON null（序列化后有 key）
        AgentUiScreen screen = AgentUiScreen.create(
            "req-null-reason",
            "<owo-ui><components><flow-layout/></components></owo-ui>",
            600, 0L
        );
        AgentUiStore.setActive(screen);

        AgentUiPayloadHandler.handleRawClose(
            "{\"request_id\":\"req-null-reason\",\"reason\":null}"
        );

        assertNull(AgentUiStore.getActive(),
            "reason=null（显式 JSON null）的 handleRawClose 匹配 request_id 后 store 应清空");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // handleRawClose — wire 对拍（server AgentUiClosePayloadV1 真实 serde 形态）
    // ─────────────────────────────────────────────────────────────────────────

    @Test
    void handleRawClose_wire_replacedPayload_hasNoReasonField() {
        // server Replaced 时序列化：{"request_id":"..."} ——  reason=None, skip_serializing_if 省略 reason 字段。
        // wire 对拍：确保 client 不依赖 reason 字段存在，缺 reason 时也能正确 close。
        AgentUiScreen screen = AgentUiScreen.create(
            "req-replaced",
            "<owo-ui><components><flow-layout/></components></owo-ui>",
            600, 0L
        );
        AgentUiStore.setActive(screen);

        // 真实 server Replaced wire JSON（Rust skip_serializing reason=None → 无 reason key）
        String serverWireReplaced = "{\"request_id\":\"req-replaced\"}";
        AgentUiPayloadHandler.handleRawClose(serverWireReplaced);

        assertNull(AgentUiStore.getActive(),
            "server Replaced wire JSON（无 reason 字段）应正确清除 active screen；"
                + "实际 getActive()=" + AgentUiStore.getActive());
    }

    @Test
    void handleRawClose_wire_sessionExpiredPayload_parsesReason() {
        // server session_expired 时序列化：{"request_id":"...", "reason":"session_expired"}
        AgentUiScreen screen = AgentUiScreen.create(
            "req-expired",
            "<owo-ui><components><flow-layout/></components></owo-ui>",
            600, 0L
        );
        AgentUiStore.setActive(screen);

        String serverWireExpired = "{\"request_id\":\"req-expired\",\"reason\":\"session_expired\"}";
        AgentUiPayloadHandler.handleRawClose(serverWireExpired);

        assertNull(AgentUiStore.getActive(),
            "server session_expired wire JSON 应正确清除 active screen；"
                + "实际 getActive()=" + AgentUiStore.getActive());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // handleRawClose — missing / blank request_id → early exit，不改 store
    // ─────────────────────────────────────────────────────────────────────────

    @Test
    void handleRawClose_missingRequestId_doesNotClearStore() {
        AgentUiScreen screen = AgentUiScreen.create(
            "req-guard",
            "<owo-ui><components><flow-layout/></components></owo-ui>",
            600, 0L
        );
        AgentUiStore.setActive(screen);

        AgentUiPayloadHandler.handleRawClose("{\"reason\":\"session_expired\"}");

        assertNotNull(AgentUiStore.getActive(),
            "缺 request_id 的 handleRawClose 不应修改 AgentUiStore（早退出保护）");
        assertEquals("req-guard", AgentUiStore.getActive().requestId(),
            "缺 request_id 后 store 的 requestId 应保持 req-guard，实际="
                + AgentUiStore.getActive().requestId());
    }

    @Test
    void handleRawClose_blankRequestId_doesNotClearStore() {
        AgentUiScreen screen = AgentUiScreen.create(
            "req-blank-guard",
            "<owo-ui><components><flow-layout/></components></owo-ui>",
            600, 0L
        );
        AgentUiStore.setActive(screen);

        AgentUiPayloadHandler.handleRawClose("{\"request_id\":\"   \"}");

        assertNotNull(AgentUiStore.getActive(),
            "空白 request_id 的 handleRawClose 不应修改 AgentUiStore（isBlank 早退出保护）");
    }

    @Test
    void handleRawClose_emptyRequestId_doesNotClearStore() {
        AgentUiScreen screen = AgentUiScreen.create(
            "req-empty-guard",
            "<owo-ui><components><flow-layout/></components></owo-ui>",
            600, 0L
        );
        AgentUiStore.setActive(screen);

        AgentUiPayloadHandler.handleRawClose("{\"request_id\":\"\"}");

        assertNotNull(AgentUiStore.getActive(),
            "空字符串 request_id 的 handleRawClose 不应修改 AgentUiStore");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // handleRawClose — JSON 解析错误 → graceful ignore
    // ─────────────────────────────────────────────────────────────────────────

    @Test
    void handleRawClose_malformedJson_doesNotThrow() {
        assertDoesNotThrow(
            () -> AgentUiPayloadHandler.handleRawClose("{not valid json!!!}"),
            "畸形 JSON 不应抛出异常（应被 try-catch 吞掉并记录日志）"
        );
        assertNull(AgentUiStore.getActive(),
            "畸形 JSON 后 store 应仍为 null");
    }

    @Test
    void handleRawClose_emptyString_doesNotThrow() {
        assertDoesNotThrow(
            () -> AgentUiPayloadHandler.handleRawClose(""),
            "空字符串不应抛出异常"
        );
    }

    @Test
    void handleRawClose_nullString_doesNotThrow() {
        assertDoesNotThrow(
            () -> AgentUiPayloadHandler.handleRawClose(null),
            "null payload 不应抛出异常"
        );
    }

    @Test
    void handleRawClose_notJsonObject_doesNotThrow() {
        // JSON array 是合法 JSON 但 getAsJsonObject() 会抛，应被捕获
        assertDoesNotThrow(
            () -> AgentUiPayloadHandler.handleRawClose("[\"req-id\"]"),
            "JSON 数组（非 object）不应抛出未捕获异常"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // handleRawClose — state transitions
    // ─────────────────────────────────────────────────────────────────────────

    @Test
    void handleRawClose_consecutiveCalls_secondNoMatchIsNoop() {
        // close req-A → store 清空；再 close req-A → 无活跃 session，幂等无 crash
        AgentUiStore.setActive(AgentUiScreen.create(
            "req-A",
            "<owo-ui><components><flow-layout/></components></owo-ui>",
            600, 0L
        ));

        AgentUiPayloadHandler.handleRawClose("{\"request_id\":\"req-A\"}");
        assertNull(AgentUiStore.getActive(), "第一次 close 后 store 应为 null");

        assertDoesNotThrow(
            () -> AgentUiPayloadHandler.handleRawClose("{\"request_id\":\"req-A\"}"),
            "第二次 close（已无活跃 session）不应抛出异常"
        );
        assertNull(AgentUiStore.getActive(), "第二次 close 后 store 仍应为 null");
    }

    @Test
    void handleRawClose_newRequestArrives_oldCloseDoesNotAffectIt() {
        // req-old 已 close，新 req-new 到来 → close req-old 不应误删 req-new
        AgentUiStore.setActive(AgentUiScreen.create(
            "req-old",
            "<owo-ui><components><flow-layout/></components></owo-ui>",
            600, 0L
        ));
        AgentUiPayloadHandler.handleRawClose("{\"request_id\":\"req-old\"}");
        assertNull(AgentUiStore.getActive(), "前提：req-old close 后 store 应为 null");

        AgentUiStore.setActive(AgentUiScreen.create(
            "req-new",
            "<owo-ui><components><flow-layout/></components></owo-ui>",
            300, 100L
        ));

        // 迟到的 close req-old → 不应误删 req-new
        AgentUiPayloadHandler.handleRawClose("{\"request_id\":\"req-old\"}");

        assertNotNull(AgentUiStore.getActive(),
            "迟到的 close req-old 不应误删新 active req-new；store 仍应非 null");
        assertEquals("req-new", AgentUiStore.getActive().requestId(),
            "新 active screen requestId 应为 req-new，实际=" + AgentUiStore.getActive().requestId());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // handleRawRequest — early-exit paths（JSON 解析前 / request_id 校验，无 MinecraftClient 依赖）
    // ─────────────────────────────────────────────────────────────────────────

    @Test
    void handleRawRequest_malformedJson_doesNotThrowAndLeavesStoreNull() {
        // 这条路径在 JsonParser.parseString 时抛异常并被捕获，不访问 client
        // 传 null client 验证：该路径不会 NPE（在解析异常时已返回）
        assertDoesNotThrow(
            () -> AgentUiPayloadHandler.handleRawRequest("{invalid json!!}", null),
            "畸形 JSON 不应抛出异常（应被 try-catch 捕获）"
        );
        assertNull(AgentUiStore.getActive(),
            "畸形 JSON 后 AgentUiStore 不应被写入");
    }

    @Test
    void handleRawRequest_missingRequestId_doesNotThrowAndLeavesStoreNull() {
        // request_id 缺失 → 在访问 client 之前已早退出
        assertDoesNotThrow(
            () -> AgentUiPayloadHandler.handleRawRequest(
                "{\"xml\":\"<owo-ui><components><flow-layout/></components></owo-ui>\","
                    + "\"timeout_ticks\":600}",
                null
            ),
            "缺 request_id 的 payload 不应抛出异常"
        );
        assertNull(AgentUiStore.getActive(),
            "缺 request_id 的 payload 不应写入 AgentUiStore");
    }

    @Test
    void handleRawRequest_blankRequestId_doesNotThrowAndLeavesStoreNull() {
        assertDoesNotThrow(
            () -> AgentUiPayloadHandler.handleRawRequest(
                "{\"request_id\":\"   \","
                    + "\"xml\":\"<owo-ui><components><flow-layout/></components></owo-ui>\","
                    + "\"timeout_ticks\":600}",
                null
            ),
            "空白 request_id 不应抛出异常"
        );
        assertNull(AgentUiStore.getActive(),
            "空白 request_id 不应写入 AgentUiStore");
    }

    @Test
    void handleRawRequest_emptyJson_doesNotThrow() {
        // {} 是合法 JSON object 但 request_id 缺失 → 早退出
        assertDoesNotThrow(
            () -> AgentUiPayloadHandler.handleRawRequest("{}", null),
            "{} payload 不应抛出异常"
        );
        assertNull(AgentUiStore.getActive(),
            "{} payload 不应写入 AgentUiStore");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // handleRawRequest — wire 对拍（server AgentUiRequestPayloadV1 真实 serde 形态）
    // ─────────────────────────────────────────────────────────────────────────

    @Test
    void handleRawRequest_wire_barePayloadHasNoEnvelopeFields_parses() {
        // server 真实 wire JSON（serde_json AgentUiRequestPayloadV1）：
        // {"request_id":"...","target_player":"...","xml":"...","timeout_ticks":600}
        // 无 "v" / "type" 字段（那是 ServerDataV1 外层）。
        // ready-client happy path 见 AgentUiPayloadHandlerReadyClientTest；此处只锁 null client
        // 在 headless JVM 中不抛异常、不写 store。
        String serverWireJson =
            "{\"request_id\":\"req-wire-bare\","
                + "\"target_player\":\"offline:Kiz\","
                + "\"xml\":\"<owo-ui><components><flow-layout><label>天道降示</label></flow-layout></components></owo-ui>\","
                + "\"timeout_ticks\":600}";

        assertDoesNotThrow(
            () -> AgentUiPayloadHandler.handleRawRequest(serverWireJson, null),
            "server wire bare JSON 传 null client 时不应靠 NPE 证明解析成功"
        );
        // AgentUiStore 不应被写入（client 为 null 时流程中止）
        assertNull(AgentUiStore.getActive(),
            "server wire bare JSON 传 null client 时 AgentUiStore 不应被写入；"
                + "实际 getActive()=" + AgentUiStore.getActive());
    }

    @Test
    void handleRawRequest_wire_noEnvelopeTypeField_withNullClientDoesNotThrow() {
        // 验证：无 "type" 字段的 server 裸 payload 在 headless/null client 下不抛异常。
        // parse→open happy path 由 ready-client stub 测试锁住。
        String barePayloadNoType =
            "{\"request_id\":\"req-no-type\",\"xml\":\"\",\"timeout_ticks\":100}";

        assertDoesNotThrow(
            () -> AgentUiPayloadHandler.handleRawRequest(barePayloadNoType, null),
            "无 type 字段的裸 payload 传 null client 不应抛 NPE"
        );
        assertNull(AgentUiStore.getActive(),
            "传 null client 时 AgentUiStore 不应被写入");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // handleRawRequest — bare-minimal / null 输入不崩
    // ─────────────────────────────────────────────────────────────────────────

    @Test
    void handleRawRequest_nullPayload_doesNotThrow() {
        assertDoesNotThrow(
            () -> AgentUiPayloadHandler.handleRawRequest(null, null),
            "null payload 不应抛出未捕获异常"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // ServerDataRouter 防回归 pin（专属 channel 迁移后不再注册）
    // ─────────────────────────────────────────────────────────────────────────

    /**
     * 防回归：AgentUiRequest/AgentUiClose 已迁移到专属 bong:agent_ui_request/bong:agent_ui_close
     * JSON channel，不再经 bong:server_data / proto 路径（proto_convert.rs 对这两个 variant 是
     * unreachable!()，生产 panic）。ServerDataRouter 不应再注册这两个 key。
     *
     * <p>若此测试失败，说明有人误将 agent_ui 迁回 server_data 路径，会导致 production proto panic。
     */
    @Test
    void defaultRouter_doesNotRegisterAgentUiRequest() {
        assertFalse(
            ServerDataRouter.createDefault().registeredTypes().contains("agent_ui_request"),
            "ServerDataRouter 不应注册 'agent_ui_request'（已迁移到专属 bong:agent_ui_request channel，"
                + "server_data 路径 production 会 proto_convert.rs unreachable!() panic）"
        );
    }

    @Test
    void defaultRouter_doesNotRegisterAgentUiClose() {
        assertFalse(
            ServerDataRouter.createDefault().registeredTypes().contains("agent_ui_close"),
            "ServerDataRouter 不应注册 'agent_ui_close'（已迁移到专属 bong:agent_ui_close channel，"
                + "server_data 路径 production 会 proto_convert.rs unreachable!() panic）"
        );
    }
}
