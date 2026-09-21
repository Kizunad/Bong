package com.bong.client.fauna;

import net.fabricmc.fabric.api.client.rendering.v1.EntityRendererRegistry;

public final class FaunaRenderBootstrap {
    private FaunaRenderBootstrap() {
    }

    public static void register() {
        FaunaEntities.register();
        registerRenderers(false);
    }

    public static void registerDeferred() {
        FaunaEntities.registerDeferred();
        registerRenderers(true);
    }

    private static void registerRenderers(boolean deferred) {
        for (FaunaVisualKind kind : FaunaVisualKind.values()) {
            if (kind.deferredRegistration() != deferred) continue;
            EntityRendererRegistry.register(FaunaEntities.type(kind), ctx -> new FaunaRenderer(ctx, kind));
        }
    }
}
