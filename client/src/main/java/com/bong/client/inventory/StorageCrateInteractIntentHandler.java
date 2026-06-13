package com.bong.client.inventory;

import com.bong.client.input.InteractCandidate;
import com.bong.client.input.IntentHandler;
import net.minecraft.client.MinecraftClient;

import java.util.Optional;

public final class StorageCrateInteractIntentHandler implements IntentHandler {
    private static final String DEBUG_PREFIX = "storage_crate:";
    private static final String TRADE_CRATE_MODEL_ID = "trade_crate";
    private static final String HERB_CRATE_MODEL_ID = "herb_crate_placed";

    @Override
    public Optional<InteractCandidate> candidate(MinecraftClient client) {
        return ContainerOpenIntentSupport.candidate(
            client,
            DEBUG_PREFIX,
            StorageCrateInteractIntentHandler::isStorageCrateModelId
        );
    }

    @Override
    public boolean dispatch(MinecraftClient client, InteractCandidate candidate) {
        return ContainerOpenIntentSupport.dispatch(client, candidate, DEBUG_PREFIX);
    }

    public static boolean isStorageCrateModelId(String modelId) {
        return TRADE_CRATE_MODEL_ID.equals(modelId) || HERB_CRATE_MODEL_ID.equals(modelId);
    }

    static int candidateEntityId(InteractCandidate candidate) {
        return ContainerOpenIntentSupport.candidateEntityId(candidate, DEBUG_PREFIX);
    }
}
