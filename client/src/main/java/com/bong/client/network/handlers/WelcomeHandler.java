package com.bong.client.network.handlers;

import com.bong.client.network.PayloadHandler;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;

public class WelcomeHandler implements PayloadHandler {
    @Override
    public void handle(MinecraftClient client, String type, String jsonPayload) {
        if (client.player != null) {
            client.player.sendMessage(Text.literal("[Bong] welcome: Server connected."), false);
        }
    }
}
