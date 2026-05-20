package com.bong.client.armor;

import dev.felnull.specialmodelloader.api.event.SpecialModelLoaderEvents;
import net.fabricmc.fabric.api.client.rendering.v1.LivingEntityFeatureRendererRegistrationCallback;
import net.minecraft.client.render.entity.PlayerEntityRenderer;
import net.minecraft.util.Identifier;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * plan-depth-loop-v1 P1: Armor render bootstrap -- SML scope + FeatureRenderer registration.
 *
 * <p>Registers {@link ArmorFeatureRenderer} via Fabric's
 * {@link LivingEntityFeatureRendererRegistrationCallback} so it attaches to all
 * {@link PlayerEntityRenderer} instances. Also registers the bong armor model
 * namespace with SML's LOAD_SCOPE so OBJ files under {@code bong:models/armor/}
 * are loaded by the special model loader.
 */
public final class ArmorRenderBootstrap {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-armor-render");
    private static final String BONG_NS = "bong";

    private ArmorRenderBootstrap() {}

    public static void register() {
        SpecialModelLoaderEvents.LOAD_SCOPE.register(ArmorRenderBootstrap::isBongArmorModel);

        LivingEntityFeatureRendererRegistrationCallback.EVENT.register(
            (entityType, entityRenderer, registrationHelper, context) -> {
                if (entityRenderer instanceof PlayerEntityRenderer playerRenderer) {
                    registrationHelper.register(new ArmorFeatureRenderer(playerRenderer));
                }
            }
        );

        LOGGER.info("ArmorRenderBootstrap: SML scope + FeatureRenderer registered, {} armor models",
            ArmorModelRegistry.size());
    }

    private static boolean isBongArmorModel(Identifier location) {
        return BONG_NS.equals(location.getNamespace())
            && location.getPath().startsWith("models/armor/");
    }
}
