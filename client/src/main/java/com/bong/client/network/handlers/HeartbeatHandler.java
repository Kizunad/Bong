package com.bong.client.network.handlers;

import com.bong.client.network.PayloadHandler;
import net.minecraft.client.MinecraftClient;

public class HeartbeatHandler implements PayloadHandler {
    @Override
    public void handle(MinecraftClient client, String type, String jsonPayload) {
        // Ignored for now, just drop on floor
    }
}
