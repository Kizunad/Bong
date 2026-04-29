package com.bong.client.social;

import com.bong.client.BongClient;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.network.ClientRequestSender;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.util.hit.EntityHitResult;
import org.lwjgl.glfw.GLFW;

import java.util.Comparator;

public final class TradeOfferScreenBootstrap {
    private static final String CATEGORY = "category.bong-client.controls";
    private static final String TRADE_KEY_TRANSLATION = "key.bong-client.social_trade";
    private static KeyBinding tradeKey;

    private TradeOfferScreenBootstrap() {
    }

    public static void register() {
        tradeKey = KeyBindingHelper.registerKeyBinding(
            new KeyBinding(TRADE_KEY_TRANSLATION, InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_G, CATEGORY)
        );
        ClientTickEvents.END_CLIENT_TICK.register(TradeOfferScreenBootstrap::onEndClientTick);
        BongClient.LOGGER.info("Registered social trade keybinding on key: G");
    }

    static void onEndClientTick(MinecraftClient client) {
        if (client == null || client.player == null) return;
        handleIncomingOffer(client);
        while (tradeKey.wasPressed()) {
            sendOfferFromCrosshair(client);
        }
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

    private static void sendOfferFromCrosshair(MinecraftClient client) {
        if (!(client.crosshairTarget instanceof EntityHitResult hit)) return;
        if (!(hit.getEntity() instanceof PlayerEntity)) return;
        InventoryItem item = firstTradeItem(InventoryStateStore.snapshot());
        if (item == null) return;
        ClientRequestSender.sendTradeOfferRequest("entity:" + hit.getEntity().getId(), item.instanceId());
    }

    private static InventoryItem firstTradeItem(InventoryModel model) {
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
