package com.bong.client.armor;

import net.fabricmc.fabric.api.client.rendering.v1.LivingEntityFeatureRendererRegistrationCallback;
import net.minecraft.client.render.entity.PlayerEntityRenderer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * plan-tarkov-backpack-v1 P4 — 穿戴背包件上身渲染注册（TPV）。
 *
 * <p>通过 Fabric {@link LivingEntityFeatureRendererRegistrationCallback} 把
 * {@link WornPackFeatureRenderer} 挂到所有 {@link PlayerEntityRenderer}，**严格参
 * {@code ArmorRenderBootstrap}（唯一已验证可用的注册先例），不参
 * {@code MutationFeatureRenderer}（从未注册的孤岛反面教材）**。
 */
public final class WornPackRenderBootstrap {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-worn-pack-render");

    private WornPackRenderBootstrap() {}

    public static void register() {
        LivingEntityFeatureRendererRegistrationCallback.EVENT.register(
            (entityType, entityRenderer, registrationHelper, context) -> {
                if (entityRenderer instanceof PlayerEntityRenderer playerRenderer) {
                    registrationHelper.register(new WornPackFeatureRenderer(playerRenderer));
                }
            }
        );

        LOGGER.info("WornPackRenderBootstrap: FeatureRenderer registered, {} worn-pack models",
            WornPackModelRegistry.size());
    }
}
