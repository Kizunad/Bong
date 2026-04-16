package com.bong.client.network;

import com.bong.client.BongClient;
import io.netty.buffer.Unpooled;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking;
import net.minecraft.network.PacketByteBuf;
import net.minecraft.util.Identifier;

import java.nio.charset.StandardCharsets;

/**
 * 向服务端 {@code bong:client_request} 通道发送 CustomPayload。
 *
 * <p>默认实现使用 {@link ClientPlayNetworking}；测试通过 {@link #setBackendForTests(Backend)}
 * 注入替身捕获发送的 JSON。</p>
 */
public final class ClientRequestSender {

    /** 可测试的发送后端 seam。 */
    @FunctionalInterface
    public interface Backend {
        void send(Identifier channel, byte[] payload);
    }

    private static final Identifier CHANNEL = new Identifier(
        ClientRequestProtocol.CHANNEL_NAMESPACE,
        ClientRequestProtocol.CHANNEL_PATH
    );

    private static final Backend DEFAULT_BACKEND = (channel, payload) -> {
        if (!ClientPlayNetworking.canSend(channel)) {
            BongClient.LOGGER.warn(
                "Cannot send {} payload: channel not registered on server",
                channel
            );
            return;
        }
        PacketByteBuf buf = new PacketByteBuf(Unpooled.buffer(payload.length));
        buf.writeBytes(payload);
        ClientPlayNetworking.send(channel, buf);
    };

    private static volatile Backend backend = DEFAULT_BACKEND;

    private ClientRequestSender() {}

    public static void sendSetMeridianTarget(ClientRequestProtocol.MeridianId meridian) {
        dispatch(ClientRequestProtocol.encodeSetMeridianTarget(meridian));
    }

    public static void sendBreakthroughRequest() {
        dispatch(ClientRequestProtocol.encodeBreakthroughRequest());
    }

    /** 顿悟决定：{@code chosenIdx = null} 表示拒绝或超时。 */
    public static void sendInsightDecision(String triggerId, Integer chosenIdx) {
        dispatch(ClientRequestProtocol.encodeInsightDecision(triggerId, chosenIdx));
    }

    public static void sendForgeRequest(
        ClientRequestProtocol.MeridianId meridian,
        ClientRequestProtocol.ForgeAxis axis
    ) {
        dispatch(ClientRequestProtocol.encodeForgeRequest(meridian, axis));
    }

    /** Combat UI 系列 C2S 通用发送入口。 */
    public static void send(String type, com.google.gson.JsonObject payload) {
        dispatch(ClientRequestProtocol.encodeGeneric(type, payload));
    }

    private static void dispatch(String json) {
        backend.send(CHANNEL, json.getBytes(StandardCharsets.UTF_8));
    }

    public static void setBackendForTests(Backend b) {
        backend = b;
    }

    public static void resetBackendForTests() {
        backend = DEFAULT_BACKEND;
    }
}
