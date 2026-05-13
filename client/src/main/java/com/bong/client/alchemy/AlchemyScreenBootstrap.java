package com.bong.client.alchemy;

import com.bong.client.BongClient;
import net.minecraft.client.MinecraftClient;
import net.minecraft.util.math.BlockPos;

public final class AlchemyScreenBootstrap {
    private AlchemyScreenBootstrap() {}

    public static void register() {
        BongClient.LOGGER.info("Registered alchemy screen bootstrap via furnace interaction");
    }

    public static void requestOpenAlchemyScreen(MinecraftClient client, BlockPos pos) {
        if (client == null || pos == null) {
            return;
        }
        client.execute(() -> {
            if (client.currentScreen instanceof AlchemyScreen) {
                return;
            }
            com.bong.client.network.ClientRequestSender.sendAlchemyOpenFurnace(pos);
            client.setScreen(new AlchemyScreen(pos));
        });
    }
}
