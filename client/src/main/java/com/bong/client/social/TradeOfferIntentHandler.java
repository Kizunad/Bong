package com.bong.client.social;

import com.bong.client.input.InteractCandidate;
import com.bong.client.input.InteractIntent;
import com.bong.client.input.IntentHandler;
import com.bong.client.input.ReservedInteractionIntents;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.network.ClientRequestSender;
import net.minecraft.client.MinecraftClient;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.util.hit.EntityHitResult;

import java.util.Optional;

public final class TradeOfferIntentHandler implements IntentHandler {
    @Override
    public Optional<InteractCandidate> candidate(MinecraftClient client) {
        EntityHitResult hit = playerHit(client);
        if (hit == null || TradeOfferScreenBootstrap.firstTradeItem(InventoryStateStore.snapshot()) == null) {
            return Optional.empty();
        }
        double distanceSq = client.player.squaredDistanceTo(hit.getEntity());
        return Optional.of(InteractCandidate.of(
            InteractIntent.TradePlayer,
            ReservedInteractionIntents.TRADE_PLAYER_PRIORITY,
            distanceSq,
            "trade_player:" + hit.getEntity().getId()
        ));
    }

    @Override
    public boolean dispatch(MinecraftClient client, InteractCandidate candidate) {
        EntityHitResult hit = playerHit(client);
        if (hit == null) {
            return false;
        }
        InventoryItem item = TradeOfferScreenBootstrap.firstTradeItem(InventoryStateStore.snapshot());
        if (item == null) {
            return false;
        }
        ClientRequestSender.sendTradeOfferRequest("entity:" + hit.getEntity().getId(), item.instanceId());
        return true;
    }

    private static EntityHitResult playerHit(MinecraftClient client) {
        if (client == null || client.player == null) {
            return null;
        }
        if (!(client.crosshairTarget instanceof EntityHitResult hit)) {
            return null;
        }
        if (!(hit.getEntity() instanceof PlayerEntity)) {
            return null;
        }
        return hit;
    }
}
