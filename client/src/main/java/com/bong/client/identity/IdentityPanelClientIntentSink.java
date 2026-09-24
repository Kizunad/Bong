package com.bong.client.identity;

import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;
import net.minecraft.client.MinecraftClient;

import java.util.Objects;

/** 将身份面板 typed intent 适配到原有客户端命令通道。 */
public final class IdentityPanelClientIntentSink implements UiIntentSink<IdentityPanelIntent> {
    private final Transport transport;

    IdentityPanelClientIntentSink(Transport transport) {
        this.transport = Objects.requireNonNull(transport, "transport must not be null");
    }

    /** 生产组合根：保留 server `/identity` 命令和既有权限校验。 */
    public static IdentityPanelClientIntentSink production() {
        return new IdentityPanelClientIntentSink(command -> {
            MinecraftClient client = MinecraftClient.getInstance();
            if (client == null || client.player == null || client.player.networkHandler == null) {
                throw new IllegalStateException("identity command requires an active player connection");
            }
            client.player.networkHandler.sendCommand(command);
        });
    }

    @Override
    public UiIntentResult dispatch(IdentityPanelIntent intent) {
        if (intent == null) {
            return UiIntentResult.rejected("identity intent must not be null");
        }
        try {
            transport.send(IdentityPanelIntent.command(intent));
            return UiIntentResult.accepted();
        } catch (RuntimeException failure) {
            String detail = failure.getMessage();
            return UiIntentResult.error("identity transport failed: "
                + (detail == null || detail.isBlank()
                    ? failure.getClass().getSimpleName() : detail));
        }
    }

    @FunctionalInterface
    interface Transport {
        void send(String command);
    }
}
