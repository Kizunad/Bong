package com.bong.client.network;

import com.bong.client.agentui.AgentUiScreen;
import com.bong.client.agentui.AgentUiStore;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.JsonPrimitive;
import net.minecraft.client.MinecraftClient;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import javax.annotation.Nullable;
import java.util.function.Consumer;

/**
 * plan-agent-ui-data-v1 P1 — 天道 UI payload handler。
 *
 * <p>处理两个 payload 类型，均通过专属 JSON channel 接收：
 * <ul>
 *   <li>{@code bong:agent_ui_request} channel 发来的裸 {@code AgentUiRequestPayloadV1} JSON
 *       （无 ServerDataV1 envelope），由 {@link #handleRawRequest(String, MinecraftClient)} 解析。</li>
 *   <li>{@code bong:agent_ui_close} channel 发来的裸 {@code AgentUiClosePayloadV1} JSON，
 *       由 {@link #handleRawClose(String)} 解析。</li>
 * </ul>
 *
 * <p>这两条路径绕开了 {@code bong:server_data} proto 路径（{@code proto_convert.rs} 对
 * AgentUiRequest/AgentUiClose 是 {@code unreachable!()}，生产会 panic），
 * 消除了 fix-s2c-proto-panic 修复的根因。
 *
 * <p>payload 字段说明：
 * <ul>
 *   <li>{@code bong:agent_ui_request}：request_id / target_player / xml / timeout_ticks</li>
 *   <li>{@code bong:agent_ui_close}：request_id, reason?（reason 为 null 时 serde skip，表示 Replaced）</li>
 * </ul>
 *
 * <p>注册入口：{@code BongNetworkHandler.registerAgentUiChannels()}。
 * ServerDataRouter 不再注册 agent_ui_request/agent_ui_close。
 */
public final class AgentUiPayloadHandler {
    private static final Logger LOGGER = LoggerFactory.getLogger(AgentUiPayloadHandler.class);

    static ServerDataDispatch openReadyRequestForTests(
        String payloadType,
        String requestId,
        String xml,
        int timeoutTicks,
        long currentTick,
        Consumer<AgentUiScreen> opener
    ) {
        int safeTimeoutTicks = timeoutTicks <= 0 ? 1 : timeoutTicks;
        AgentUiScreen screen = AgentUiScreen.create(
            requestId,
            xml,
            safeTimeoutTicks,
            currentTick
        );
        opener.accept(screen);
        return ServerDataDispatch.handled(
            payloadType,
            "agent_ui_request 已打开面板 request_id='" + requestId + "'"
        );
    }

    static ServerDataDispatch handleRawRequestForReadyClientTests(
        String jsonPayload,
        long currentTick,
        Consumer<AgentUiScreen> opener
    ) {
        ServerDataDispatch dispatch = parseAndMaybeOpenRawRequest(
            jsonPayload,
            true,
            currentTick,
            opener
        );
        if (dispatch == null) {
            return ServerDataDispatch.noOp(
                "agent_ui_request",
                "agent_ui_request payload ignored by ready-client test seam"
            );
        }
        return dispatch;
    }

    // ─── JSON helpers ────────────────────────────────────────────────────────

    @Nullable
    private static String readString(JsonObject obj, String field) {
        var el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) {
            return null;
        }
        JsonPrimitive prim = el.getAsJsonPrimitive();
        if (!prim.isString()) {
            return null;
        }
        return prim.getAsString();
    }

    private static int readInt(JsonObject obj, String field, int fallback) {
        var el = obj.get(field);
        if (el == null || el.isJsonNull() || !el.isJsonPrimitive()) {
            return fallback;
        }
        JsonPrimitive prim = el.getAsJsonPrimitive();
        if (!prim.isNumber()) {
            return fallback;
        }
        try {
            return prim.getAsInt();
        } catch (NumberFormatException e) {
            return fallback;
        }
    }

    private static long getClientTick(MinecraftClient client) {
        // 用 world time 近似当前 tick；若 world 不可用则返回 0
        if (client.world != null) {
            return client.world.getTime();
        }
        return 0L;
    }

    // ─── 专属 channel 原始 JSON 解析（bong:agent_ui_request / bong:agent_ui_close）─────────

    /**
     * 解析来自 {@code bong:agent_ui_request} 专属 channel 的裸 JSON payload。
     *
     * <p>payload 格式（无 ServerDataV1 envelope）：
     * <pre>{@code
     * {"request_id":"...", "target_player":"...", "xml":"...", "timeout_ticks":600}
     * }</pre>
     *
     * <p>共用 {@link #openReadyRequestForTests} 逻辑；
     * 直接解析 JsonObject（绕开 ServerDataEnvelope.parse）。
     *
     * @param jsonPayload 裸 JSON 字符串（来自 bong:agent_ui_request channel）
     * @param client      当前 MinecraftClient 实例（由 channel listener 传入，已在主线程执行）
     */
    public static void handleRawRequest(String jsonPayload, MinecraftClient client) {
        parseAndMaybeOpenRawRequest(
            jsonPayload,
            client != null && client.player != null,
            client == null ? 0L : getClientTick(client),
            screen -> {
                AgentUiStore.setActive(screen);
                client.setScreen(screen);
            }
        );
    }

    @Nullable
    private static ServerDataDispatch parseAndMaybeOpenRawRequest(
        String jsonPayload,
        boolean playerReady,
        long currentTick,
        Consumer<AgentUiScreen> opener
    ) {
        JsonObject payload;
        try {
            payload = JsonParser.parseString(jsonPayload).getAsJsonObject();
        } catch (Exception e) {
            LOGGER.error("[bong][agent_ui] bong:agent_ui_request payload parse error: {}", e.getMessage());
            return null;
        }

        String requestId = readString(payload, "request_id");
        if (requestId == null || requestId.isBlank()) {
            LOGGER.warn("[bong][agent_ui] bong:agent_ui_request: 'request_id' 缺失，payload 忽略");
            return null;
        }

        String xml = readString(payload, "xml");
        if (xml == null) {
            xml = "";
        }

        int timeoutTicks = readInt(payload, "timeout_ticks", 600);
        if (timeoutTicks <= 0) {
            LOGGER.warn(
                "[bong][agent_ui] bong:agent_ui_request timeout_ticks={} 非法，回退为 1 tick request_id={}",
                timeoutTicks, requestId
            );
            timeoutTicks = 1;
        }

        if (!playerReady) {
            LOGGER.warn("[bong][agent_ui] bong:agent_ui_request: player 未就绪，payload 忽略 request_id='{}'",
                requestId);
            return null;
        }

        return openReadyRequestForTests(
            "agent_ui_request",
            requestId,
            xml,
            timeoutTicks,
            currentTick,
            opener
        );
    }

    /**
     * 解析来自 {@code bong:agent_ui_close} 专属 channel 的裸 JSON payload。
     *
     * <p>payload 格式（无 ServerDataV1 envelope）：
     * <pre>{@code
     * {"request_id":"...", "reason":null}
     * }</pre>
     *
     * <p>共用 {@link AgentUiStore#receiveClose} 逻辑。
     *
     * @param jsonPayload 裸 JSON 字符串（来自 bong:agent_ui_close channel）
     */
    public static void handleRawClose(String jsonPayload) {
        JsonObject payload;
        try {
            payload = JsonParser.parseString(jsonPayload).getAsJsonObject();
        } catch (Exception e) {
            LOGGER.error("[bong][agent_ui] bong:agent_ui_close payload parse error: {}", e.getMessage());
            return;
        }

        String requestId = readString(payload, "request_id");
        if (requestId == null || requestId.isBlank()) {
            LOGGER.warn("[bong][agent_ui] bong:agent_ui_close: 'request_id' 缺失，payload 忽略");
            return;
        }

        @Nullable String reason = readString(payload, "reason");
        AgentUiStore.receiveClose(requestId, reason);
        LOGGER.debug("[bong][agent_ui] bong:agent_ui_close handled request_id='{}' reason={}", requestId, reason);
    }
}
