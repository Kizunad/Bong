package com.bong.client.network;

import net.minecraft.client.MinecraftClient;

public interface PayloadHandler {
    void handle(MinecraftClient client, String type, String jsonPayload);
}
