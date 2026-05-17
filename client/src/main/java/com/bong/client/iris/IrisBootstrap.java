package com.bong.client.iris;

import com.bong.client.BongClient;
import com.bong.client.network.ServerDataEnvelope;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking;
import net.minecraft.util.Identifier;

public final class IrisBootstrap {
    private IrisBootstrap() {
    }

    public static void register() {
        BongIrisCompat.init();
        BongShaderCommand.register();
        registerShaderStateChannel();
        ClientTickEvents.END_CLIENT_TICK.register(client -> BongShaderState.tickInterpolate());
        ClientPlayConnectionEvents.DISCONNECT.register(
                (handler, client) -> client.execute(BongShaderState::reset));
    }

    private static void registerShaderStateChannel() {
        ClientPlayNetworking.registerGlobalReceiver(
                new Identifier(ShaderStateHandler.CHANNEL_NAMESPACE, ShaderStateHandler.CHANNEL_PATH),
                (client, handler, buf, responseSender) -> {
                    int readableBytes = buf.readableBytes();
                    byte[] bytes = new byte[readableBytes];
                    buf.readBytes(bytes);

                    String jsonPayload = ServerDataEnvelope.decodeUtf8(bytes);
                    client.execute(() -> {
                        boolean handled = ShaderStateHandler.handle(jsonPayload);
                        if (!handled) {
                            BongClient.LOGGER.warn("[BongIris] Ignoring bong:shader_state payload ({} bytes)", readableBytes);
                        }
                    });
                }
        );
    }
}
