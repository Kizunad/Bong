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
                if (session instanceof LootContainerStateStore.OpenSession) {
                    // Open InspectScreen if not already open — it will detect the loot session
                    // and mount the loot panel. If already open, its listener picks up the change.
                    if (!(client.currentScreen instanceof InspectScreen)) {
                        InspectScreen screen = InspectScreenBootstrap.createScreenForCurrentState();
                        if (screen != null) {
                            client.setScreen(screen);
                        }
                    }
                }
                // Don't close on Closed — InspectScreen handles unmounting the panel internally
            });
        });

        ClientPlayConnectionEvents.DISCONNECT.register((handler, mc) ->
            mc.execute(LootContainerStateStore::clear)
        );

        BongClient.LOGGER.info("Registered loot container screen bootstrap");
    }
}
