package com.bong.client;

import net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking;
import net.minecraft.util.Identifier;

import java.nio.charset.StandardCharsets;

public class BongNetworkHandler {
    public static final int EXPECTED_VERSION = 1;
    private static final Identifier SERVER_DATA_CHANNEL = new Identifier("bong", "server_data");

    public static void register() {
        ClientPlayNetworking.registerGlobalReceiver(SERVER_DATA_CHANNEL, (client, handler, buf, responseSender) -> {
            int readableBytes = buf.readableBytes();
            byte[] bytes = new byte[readableBytes];
            buf.readBytes(bytes);

            String jsonPayload = new String(bytes, StandardCharsets.UTF_8);
            ParseResult result = parseServerPayload(jsonPayload);

            if (!result.success) {
                BongClient.LOGGER.warn("Ignoring bong:server_data payload: {}", result.errorMessage);
                return;
            }

            client.execute(() -> {
                try {
                    BongServerPayloadRouter.route(client, result.payload);
                } catch (RuntimeException exception) {
                    BongClient.LOGGER.error(
                            "Failed to route bong:server_data payload type={}",
                            result.payload.type(),
                            exception
                    );
                }
            });
        });
    }

    public static ParseResult parseServerPayload(String jsonPayload) {
        return BongServerPayloadParser.parse(jsonPayload);
    }

    public static class ParseResult {
        public final boolean success;
        public final BongServerPayload payload;
        public final String errorMessage;

        private ParseResult(boolean success, BongServerPayload payload, String errorMessage) {
            this.success = success;
            this.payload = payload;
            this.errorMessage = errorMessage;
        }

        public static ParseResult success(BongServerPayload payload) {
            return new ParseResult(true, payload, null);
        }

        public static ParseResult error(String errorMessage) {
            return new ParseResult(false, null, errorMessage);
        }
    }
}
