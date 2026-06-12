package com.bong.client.network;

import com.bong.client.agentui.AgentUiScreen;
import com.bong.client.agentui.AgentUiStore;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import net.minecraft.client.MinecraftClient;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import javax.annotation.Nullable;
import java.util.function.Consumer;

/**
 * plan-agent-ui-data-v1 P1 — 天道 UI ServerData handler。
 *
 * <p>监听两个类型：
 * <ul>
 *   <li>{@code agent_ui_request}：解析 {@code AgentUiRequestPayloadV1}（request_id / xml / timeout_ticks），
 *       构建 {@link AgentUiScreen} 并通过 {@link MinecraftClient#setScreen} 打开</li>
 *   <li>{@code agent_ui_close}：解析 {@code AgentUiClosePayloadV1}（request_id, reason?），
 *       通知 {@link AgentUiStore} 关闭对应 session</li>
 * </ul>
 *
 * <p>两个 payload 类型共用同一 handler 实例；由 {@link ServerDataRouter} 注册。
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
}
