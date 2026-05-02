package com.bong.client.input;

import net.minecraft.client.MinecraftClient;

import java.util.Optional;

public interface IntentHandler {
    Optional<InteractCandidate> candidate(MinecraftClient client);

    boolean dispatch(MinecraftClient client, InteractCandidate candidate);
}
