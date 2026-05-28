package com.bong.client.inventory;

import com.bong.client.BongClient;
import com.bong.client.hud.LootContainerStateStore;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents;
import net.minecraft.client.MinecraftClient;

public final class LootContainerScreenBootstrap {
    private LootContainerScreenBootstrap() {}

    public static void register() {
        LootContainerStateStore.addListener(session -> {
            MinecraftClient client = MinecraftClient.getInstance();
            client.execute(() -> {
                if (session instanceof LootContainerStateStore.OpenSession open) {
                    if (!(client.currentScreen instanceof LootContainerScreen)) {
                        client.setScreen(new LootContainerScreen(open));
                    }
                } else if (session instanceof LootContainerStateStore.Closed) {
                    if (client.currentScreen instanceof LootContainerScreen) {
                        client.setScreen(null);
                    }
                }
            });
        });

        ClientPlayConnectionEvents.DISCONNECT.register((handler, mc) ->
            mc.execute(LootContainerStateStore::clear)
        );

        BongClient.LOGGER.info("Registered loot container screen bootstrap");
    }
}
