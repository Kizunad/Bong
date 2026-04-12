package com.bong.client;

import com.bong.client.insight.ClientRequestInsightDispatcher;
import com.bong.client.insight.InsightOfferScreenBootstrap;
import com.bong.client.insight.InsightOfferStore;
import com.bong.client.inventory.InspectScreenBootstrap;
import com.bong.client.ui.CultivationScreenBootstrap;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.rendering.v1.HudRenderCallback;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

public class BongClient implements ClientModInitializer {
    public static final Logger LOGGER = LoggerFactory.getLogger("bong-client");

    @Override
    public void onInitializeClient() {
        LOGGER.info("Initializing Bong Client...");

        BongNetworkHandler.register();
        HudRenderCallback.EVENT.register(BongHud::render);
        CultivationScreenBootstrap.register();
        InspectScreenBootstrap.register();
        InsightOfferScreenBootstrap.register();
        InsightOfferStore.setDispatcher(new ClientRequestInsightDispatcher());

        LOGGER.info("Bong Client bootstrap ready: network, HUD, and keybinding scheduler are active.");
    }
}
