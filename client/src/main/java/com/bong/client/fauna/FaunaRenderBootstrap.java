package com.bong.client.fauna;

import net.fabricmc.fabric.api.client.rendering.v1.EntityRendererRegistry;

public final class FaunaRenderBootstrap {
    private FaunaRenderBootstrap() {
    }

    public static void register() {
        FaunaEntities.register();
        for (FaunaVisualKind kind : FaunaVisualKind.values()) {
            EntityRendererRegistry.register(FaunaEntities.type(kind), ctx -> new FaunaRenderer(ctx, kind));
        }
    }
}
