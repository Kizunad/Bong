package com.bong.client.inventory;

import com.bong.client.input.InteractCandidate;
import com.bong.client.input.IntentHandler;
import net.minecraft.client.MinecraftClient;

import java.util.Optional;

public final class DeadDropInteractIntentHandler implements IntentHandler {
    private static final String DEBUG_PREFIX = "dead_drop:";
    private static final String DEAD_DROP_MODEL_ID = "dead_drop_box";

    @Override
    public Optional<InteractCandidate> candidate(MinecraftClient client) {
        return ContainerOpenIntentSupport.candidate(
            client,
            DEBUG_PREFIX,
            DeadDropInteractIntentHandler::isDeadDropModelId
        );
    }

    @Override
    public boolean dispatch(MinecraftClient client, InteractCandidate candidate) {
        return ContainerOpenIntentSupport.dispatch(client, candidate, DEBUG_PREFIX);
    }

    public static boolean isDeadDropModelId(String modelId) {
        return DEAD_DROP_MODEL_ID.equals(modelId);
    }

    static int candidateEntityId(InteractCandidate candidate) {
        return ContainerOpenIntentSupport.candidateEntityId(candidate, DEBUG_PREFIX);
    }
}
