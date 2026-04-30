package com.bong.client.botany;

import net.fabricmc.fabric.api.client.rendering.v1.EntityRendererRegistry;

public final class BotanyPlantRenderBootstrap {
    private BotanyPlantRenderBootstrap() {}

    public static void register() {
        BotanyPlantV2Entities.register();
        EntityRendererRegistry.register(
            BotanyPlantV2Entities.botanyPlantV2(),
            BotanyPlantEntityRenderer::new
        );
    }
}
