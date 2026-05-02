package com.bong.client.inventory;

import com.bong.client.BongClient;

public final class DroppedItemPickupBootstrap {
    private DroppedItemPickupBootstrap() {}

    public static void register() {
        BongClient.LOGGER.info("Dropped loot pickup state ready; interaction is routed through unified G key");
    }
}
