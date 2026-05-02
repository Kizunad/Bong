package com.bong.client.social;

import com.bong.client.BongClient;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.network.ClientRequestSender;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;

import java.util.Comparator;

public final class TradeOfferScreenBootstrap {
    private TradeOfferScreenBootstrap() {
    }

    public static void register() {
        ClientTickEvents.END_CLIENT_TICK.register(TradeOfferScreenBootstrap::onEndClientTick);
        BongClient.LOGGER.info("Registered incoming social trade screen tick; outgoing trade uses unified G key");
    }

    static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) return;
        handleIncomingOffer(client);
    }

    private static void handleIncomingOffer(MinecraftClient client) {
        SocialStateStore.TradeOffer offer = SocialStateStore.tradeOffer();
        Screen current = client.currentScreen;
        if (offer == null) {
            if (current instanceof TradeOfferScreen) {
                client.setScreen(null);
            }
            return;
        }
        if (offer.expiresAtMs() <= System.currentTimeMillis()) {
            ClientRequestSender.sendTradeOfferResponse(offer.offerId(), false, null);
            SocialStateStore.clearTradeOffer(offer.offerId());
            if (current instanceof TradeOfferScreen) {
                client.setScreen(null);
            }
            return;
        }
        if (!(current instanceof TradeOfferScreen screen)
            || !screen.offerIdForTests().equals(offer.offerId())) {
            if (current != null && !(current instanceof TradeOfferScreen)) return;
            client.setScreen(new TradeOfferScreen(offer));
        }
    }

    static InventoryItem firstTradeItem(InventoryModel model) {
        if (model == null) return null;
        return model.gridItems().stream()
            .map(InventoryModel.GridEntry::item)
            .filter(item -> item != null && !item.isEmpty() && item.instanceId() > 0)
            .min(Comparator.comparing(InventoryItem::displayName).thenComparingLong(InventoryItem::instanceId))
            .orElseGet(() -> model.hotbar().stream()
                .filter(item -> item != null && !item.isEmpty() && item.instanceId() > 0)
                .min(Comparator.comparing(InventoryItem::displayName).thenComparingLong(InventoryItem::instanceId))
                .orElse(null));
    }
}
