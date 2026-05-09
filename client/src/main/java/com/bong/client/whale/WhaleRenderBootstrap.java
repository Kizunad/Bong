package com.bong.client.whale;

import net.fabricmc.fabric.api.client.rendering.v1.EntityRendererRegistry;

public final class WhaleRenderBootstrap {
    private WhaleRenderBootstrap() {}

    public static void register() {
        WhaleEntities.register();
        EntityRendererRegistry.register(WhaleEntities.whale(), WhaleRenderer::new);
    }
}
