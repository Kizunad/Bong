package com.bong.client.dandao;

import net.fabricmc.fabric.api.client.rendering.v1.EntityRendererRegistry;

/**
 * plan-dandao-path-v1 P4 -- Registers Baolongwang entity type + renderer.
 *
 * <p>Must be called after {@code BongEntityRenderBootstrap.register()} in
 * {@code BongClient.onInitializeClient()} to maintain raw_id order.
 */
public final class BaolongwangRenderBootstrap {
    private BaolongwangRenderBootstrap() {}

    public static void register() {
        BaolongwangEntities.register();
        EntityRendererRegistry.register(BaolongwangEntities.baolongwang(), BaolongwangRenderer::new);
    }
}
