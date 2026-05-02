package com.bong.client.input;

import com.bong.client.inventory.DroppedItemPickupIntentHandler;
import com.bong.client.social.TradeOfferIntentHandler;
import com.bong.client.tsy.TsyContainerSearchIntentHandler;

public final class DefaultInteractionHandlers {
    private DefaultInteractionHandlers() {
    }

    public static void registerDefaults() {
        InteractKeyRouter router = InteractKeyRouter.global();
        router.register(new TsyContainerSearchIntentHandler());
        router.register(new TradeOfferIntentHandler());
        router.register(new DroppedItemPickupIntentHandler());
    }
}
