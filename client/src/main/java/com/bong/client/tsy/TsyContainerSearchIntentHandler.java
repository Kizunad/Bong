package com.bong.client.tsy;

import com.bong.client.input.InteractCandidate;
import com.bong.client.input.InteractIntent;
import com.bong.client.input.IntentHandler;
import com.bong.client.input.ReservedInteractionIntents;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.client.MinecraftClient;

import java.util.Optional;

public final class TsyContainerSearchIntentHandler implements IntentHandler {
    public static final double MAX_INTERACT_DISTANCE = 3.0;

    @Override
    public Optional<InteractCandidate> candidate(MinecraftClient client) {
        TsyContainerView container = nearest(client);
        if (container == null) {
            return Optional.empty();
        }
        return Optional.of(InteractCandidate.of(
            InteractIntent.SearchContainer,
            ReservedInteractionIntents.SEARCH_CONTAINER_PRIORITY,
            container.distanceSq(client.player.getX(), client.player.getY(), client.player.getZ()),
            "tsy_container:" + container.entityId()
        ));
    }

    @Override
    public boolean dispatch(MinecraftClient client, InteractCandidate candidate) {
        TsyContainerView container = nearest(client);
        if (container == null) {
            return false;
        }
        ClientRequestSender.sendStartSearch(container.entityId());
        return true;
    }

    private static TsyContainerView nearest(MinecraftClient client) {
        if (client == null || client.player == null) {
            return null;
        }
        return TsyContainerStateStore.nearestInteractable(client.player, MAX_INTERACT_DISTANCE);
    }
}
