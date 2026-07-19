package com.bong.client.armor;

import net.fabricmc.fabric.api.client.rendering.v1.LivingEntityFeatureRendererRegistrationCallback;
import net.minecraft.client.render.entity.PlayerEntityRenderer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * plan-armor-model-render-v1 P3: vanilla ModelPart armor FeatureRenderer registration.
 *
 * <p>Registers {@link ArmorFeatureRenderer} via Fabric's
 * {@link LivingEntityFeatureRendererRegistrationCallback} so it attaches to all
 * {@link PlayerEntityRenderer} instances. Armor no longer participates in SML's item-model scope:
 * runtime geometry comes exclusively from {@link ArmorPartModel} cube tables.
 */
public final class ArmorRenderBootstrap {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-armor-render");

    private ArmorRenderBootstrap() {}

    public static void register() {
        LivingEntityFeatureRendererRegistrationCallback.EVENT.register(
            (entityType, entityRenderer, registrationHelper, context) -> {
                if (entityRenderer instanceof PlayerEntityRenderer playerRenderer) {
                    registrationHelper.register(new ArmorFeatureRenderer(playerRenderer));
                }
            }
        );

        LOGGER.info("ArmorRenderBootstrap: ModelPart FeatureRenderer registered, {} armor models",
            ArmorModelRegistry.size());
    }
}
