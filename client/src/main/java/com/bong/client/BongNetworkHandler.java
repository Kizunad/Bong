package com.bong.client;

import com.bong.client.network.ServerDataDispatch;
import com.bong.client.network.ServerDataEnvelope;
import com.bong.client.network.ServerDataRouter;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

public class BongNetworkHandler {
    private static final ServerDataRouter ROUTER = ServerDataRouter.createDefault();
    private static final long UNKNOWN_LOG_THROTTLE_MS = 30_000L;
    private static final Map<String, Long> UNKNOWN_TYPE_LOG_TIMES = new ConcurrentHashMap<>();

    public static void register() {
        ClientPlayNetworking.registerGlobalReceiver(new Identifier("bong", "server_data"), (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);

            String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
            ServerDataRouter.RouteResult result = ROUTER.route(jsonPayload, readableBytes);

            if (result.isParseError()) {
                BongClient.LOGGER.error("Failed to parse bong:server_data payload: {}", result.logMessage());
                return;
            }

            ServerDataDispatch dispatch = result.dispatch();
            if (dispatch == null) {
                BongClient.LOGGER.warn("Ignoring bong:server_data payload without dispatch result");
                return;
            }

            if (result.isNoOp()) {
                logNoOp(result);
                return;
            }

            BongClient.LOGGER.info("Processed bong:server_data payload: {}", result.logMessage());
            dispatch.legacyMessage().ifPresent(message -> client.execute(() -> {
                if (client.player != null) {
                    client.player.sendMessage(Text.literal("[Bong] " + result.envelope().type() + ": " + message), false);
                }
            }));
        });
    }

    private static void logNoOp(ServerDataRouter.RouteResult result) {
        String payloadType = result.envelope() != null ? result.envelope().type() : "unknown";
        if (shouldLogNoOp(payloadType)) {
            BongClient.LOGGER.warn("Ignoring bong:server_data payload: {}", result.logMessage());
        }
    }

    static boolean shouldLogNoOp(String payloadType) {
        long now = System.currentTimeMillis();
        Long previous = UNKNOWN_TYPE_LOG_TIMES.put(payloadType, now);
        return previous == null || now - previous >= UNKNOWN_LOG_THROTTLE_MS;
    }
}
