package com.bong.client.inventory;

import com.bong.client.entity.BongEntityModelKind;
import com.bong.client.input.InteractCandidate;
import com.bong.client.input.IntentHandler;
import net.minecraft.client.MinecraftClient;

import java.util.Optional;

public final class StorageCrateInteractIntentHandler implements IntentHandler {
    private static final String DEBUG_PREFIX = "storage_crate:";

    @Override
    public Optional<InteractCandidate> candidate(MinecraftClient client) {
        return ContainerOpenIntentSupport.candidate(
            client,
            DEBUG_PREFIX,
            StorageCrateInteractIntentHandler::isStorageCrateKind
        );
    }

    @Override
    public boolean dispatch(MinecraftClient client, InteractCandidate candidate) {
        return ContainerOpenIntentSupport.dispatch(client, candidate, DEBUG_PREFIX);
    }

    public static boolean isStorageCrateKind(BongEntityModelKind kind) {
        return kind == BongEntityModelKind.TRADE_CRATE
            || kind == BongEntityModelKind.HERB_CRATE_PLACED;
    }

    static int candidateEntityId(InteractCandidate candidate) {
        return ContainerOpenIntentSupport.candidateEntityId(candidate, DEBUG_PREFIX);
    }
}
