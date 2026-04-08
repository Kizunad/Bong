package com.bong.client.network.handlers;

import com.bong.client.BongClient;
import com.bong.client.NarrationToastState;
import com.bong.client.network.PayloadHandler;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.MutableText;
import net.minecraft.text.Text;
import net.minecraft.text.TextColor;

import java.util.List;

public class NarrationHandler implements PayloadHandler {
    @Override
    public void handle(MinecraftClient client, String type, String jsonPayload) {
        handlePayload(jsonPayload, new ClientNarrationOutput(client));
    }

    void handlePayload(String jsonPayload, NarrationOutput output) {
        NarrationPayloadParser.ParseResult result = NarrationPayloadParser.parse(jsonPayload);
        if (!result.success()) {
            BongClient.LOGGER.warn("Ignoring malformed narration payload: {}", result.errorMessage());
            return;
        }

        dispatchNarrations(result.narrations(), output);
    }

    static void dispatchNarrations(List<NarrationPayloadParser.RenderedNarration> narrations, NarrationOutput output) {
        for (NarrationPayloadParser.RenderedNarration narration : narrations) {
            if (narration == null) {
                continue;
            }

            output.sendChat(buildChatMessage(narration));

            if (narration.toast() != null) {
                output.showToast(narration.toast());
            }
        }
    }

    static Text buildChatMessage(NarrationPayloadParser.RenderedNarration narration) {
        MutableText message = Text.empty();

        if (narration.chatLabel() != null && !narration.chatLabel().isBlank()) {
            message.append(
                Text.literal(narration.chatLabel()).styled(style -> style
                    .withColor(TextColor.fromRgb(narration.labelColor()))
                    .withBold(narration.boldLabel()))
            );
            message.append(Text.literal(" "));
        }

        message.append(
            Text.literal(narration.text()).styled(style -> style.withColor(TextColor.fromRgb(narration.textColor())))
        );

        return message;
    }

    interface NarrationOutput {
        void sendChat(Text message);

        void showToast(NarrationPayloadParser.ToastSpec toast);
    }

    private static final class ClientNarrationOutput implements NarrationOutput {
        private final MinecraftClient client;

        private ClientNarrationOutput(MinecraftClient client) {
            this.client = client;
        }

        @Override
        public void sendChat(Text message) {
            if (client.player != null) {
                client.player.sendMessage(message, false);
            }
        }

        @Override
        public void showToast(NarrationPayloadParser.ToastSpec toast) {
            NarrationToastState.show(toast.text(), toast.color(), toast.durationMs());
        }
    }
}
