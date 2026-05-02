package com.bong.client.inventory;

import com.bong.client.input.InteractCandidate;
import com.bong.client.input.InteractIntent;
import com.bong.client.input.IntentHandler;
import com.bong.client.input.ReservedInteractionIntents;
import com.bong.client.inventory.state.DroppedItemStore;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.client.MinecraftClient;

import java.util.Optional;

public final class DroppedItemPickupIntentHandler implements IntentHandler {
    @Override
    public Optional<InteractCandidate> candidate(MinecraftClient client) {
        DroppedItemStore.Entry nearest = nearest(client);
        if (nearest == null) {
            return Optional.empty();
        }
        double distanceSq = distanceSq(client, nearest);
        return Optional.of(InteractCandidate.of(
            InteractIntent.PickupDroppedItem,
            ReservedInteractionIntents.PICKUP_DROPPED_ITEM_PRIORITY,
            distanceSq,
            "dropped_loot:" + nearest.instanceId()
        ));
    }

    @Override
    public boolean dispatch(MinecraftClient client, InteractCandidate candidate) {
        DroppedItemStore.Entry nearest = nearest(client);
        if (nearest == null) {
            return false;
        }
        ClientRequestSender.sendPickupDroppedItem(nearest.instanceId());
        return true;
    }

    private static DroppedItemStore.Entry nearest(MinecraftClient client) {
        if (client == null || client.player == null) {
            return null;
        }
        return DroppedItemStore.nearestTo(
            client.player.getX(),
            client.player.getY(),
            client.player.getZ()
        );
    }

    private static double distanceSq(MinecraftClient client, DroppedItemStore.Entry entry) {
        double dx = client.player.getX() - entry.worldPosX();
        double dy = client.player.getY() - entry.worldPosY();
        double dz = client.player.getZ() - entry.worldPosZ();
        return dx * dx + dy * dy + dz * dz;
    }
}
