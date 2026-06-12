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
 * <p>处理两个 payload 类型，通过两条路径接收：
 * <ol>
 *   <li><strong>专属 JSON channel（生产路径）</strong>：{@code bong:agent_ui_request} /
 *       {@code bong:agent_ui_close} channel 发来的裸 JSON（无 ServerDataV1 envelope），
 *       由 {@link #handleRawRequest(String, MinecraftClient)} / {@link #handleRawClose(String)}
 *       直接解析——这是正确的路径，绕开 proto_convert.rs 的 unreachable!() 分支。</li>
 *   <li><strong>ServerDataRouter 路由（已废弃，仅保留测试兼容）</strong>：曾经通过
 *       {@code bong:server_data} channel 发送，在 proto 路径下 production 会 panic。
 *       ServerDataRouter 不再注册 agent_ui_request/agent_ui_close。</li>
 * </ol>
 *
 * <p>payload 字段说明：
 * <ul>
 *   <li>{@code agent_ui_request}：request_id / xml / timeout_ticks（+ target_player）</li>
 *   <li>{@code agent_ui_close}：request_id, reason?</li>
 * </ul>
 */
public final class AgentUiPayloadHandler implements ServerDataHandler {
    private static final Logger LOGGER = LoggerFactory.getLogger(AgentUiPayloadHandler.class);

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        return switch (envelope.type()) {
            case "agent_ui_request" -> handleRequest(envelope);
            case "agent_ui_close"   -> handleClose(envelope);
            default -> ServerDataDispatch.noOp(
                envelope.type(),
                "AgentUiPayloadHandler: unknown type '" + envelope.type() + "'"
            );
        };
    }

    // ─── agent_ui_request ───────────────────────────────────────────────────

    private static ServerDataDispatch handleRequest(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();

        String requestId = readString(payload, "request_id");
        if (requestId == null || requestId.isBlank()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "agent_ui_request: 'request_id' 缺失，payload 忽略"
            );
        }

        String xml = readString(payload, "xml");
        if (xml == null) {
            xml = "";
        }

        int timeoutTicks = readInt(payload, "timeout_ticks", 600);
        if (timeoutTicks <= 0) {
            LOGGER.warn(
                "[bong][agent_ui] agent_ui_request timeout_ticks={} 非法，回退为 1 tick request_id={}",
                timeoutTicks,
                requestId
            );
            timeoutTicks = 1;
        }

        final String finalXml = xml;
        final String finalRequestId = requestId;
        final int finalTimeoutTicks = timeoutTicks;

        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null || client.player == null) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "agent_ui_request: MinecraftClient/player 未就绪，payload 忽略 request_id='" + finalRequestId + "'"
            );
        }

        client.execute(() -> {
            if (client.player == null) {
                return;
            }
            openReadyRequestForTests(
                envelope.type(),
                finalRequestId,
                finalXml,
                finalTimeoutTicks,
                getClientTick(client),
                screen -> {
                    AgentUiStore.setActive(screen);
                    client.setScreen(screen);
                }
            );
        });

        return ServerDataDispatch.handled(
            envelope.type(),
            "agent_ui_request 已打开面板 request_id='" + finalRequestId + "'"
        );
    }

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

    // ─── agent_ui_close ─────────────────────────────────────────────────────

    private static ServerDataDispatch handleClose(ServerDataEnvelope envelope) {
        JsonObject payload = envelope.payload();

        String requestId = readString(payload, "request_id");
        if (requestId == null || requestId.isBlank()) {
            return ServerDataDispatch.noOp(
                envelope.type(),
                "agent_ui_close: 'request_id' 缺失，payload 忽略"
            );
        }

        @Nullable String reason = readString(payload, "reason");

        AgentUiStore.receiveClose(requestId, reason);

        return ServerDataDispatch.handled(
            envelope.type(),
            "agent_ui_close 已处理 request_id='" + requestId + "' reason=" + reason
        );
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
     * <p>与 {@link #handleRequest(ServerDataEnvelope)} 共用 {@link #openReadyRequestForTests} 逻辑；
     * 入口改为直接解析 JsonObject（绕开 ServerDataEnvelope.parse）。
     *
     * @param jsonPayload 裸 JSON 字符串（来自 bong:agent_ui_request channel）
     * @param client      当前 MinecraftClient 实例（由 channel listener 传入，已在主线程执行）
     */
    public static void handleRawRequest(String jsonPayload, MinecraftClient client) {
        JsonObject payload;
        try {
            payload = JsonParser.parseString(jsonPayload).getAsJsonObject();
        } catch (Exception e) {
            LOGGER.error("[bong][agent_ui] bong:agent_ui_request payload parse error: {}", e.getMessage());
            return;
        }

        String requestId = readString(payload, "request_id");
        if (requestId == null || requestId.isBlank()) {
            LOGGER.warn("[bong][agent_ui] bong:agent_ui_request: 'request_id' 缺失，payload 忽略");
            return;
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

        if (client.player == null) {
            LOGGER.warn("[bong][agent_ui] bong:agent_ui_request: player 未就绪，payload 忽略 request_id='{}'",
                requestId);
            return;
        }

        final String finalRequestId = requestId;
        final String finalXml = xml;
        final int finalTimeoutTicks = timeoutTicks;
        openReadyRequestForTests(
            "agent_ui_request",
            finalRequestId,
            finalXml,
            finalTimeoutTicks,
            getClientTick(client),
            screen -> {
                AgentUiStore.setActive(screen);
                client.setScreen(screen);
            }
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
     * <p>与 {@link #handleClose(ServerDataEnvelope)} 共用 {@link AgentUiStore#receiveClose} 逻辑。
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
