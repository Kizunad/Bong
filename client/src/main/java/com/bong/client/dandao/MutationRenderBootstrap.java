package com.bong.client.dandao;

import net.fabricmc.fabric.api.client.rendering.v1.LivingEntityFeatureRendererRegistrationCallback;
import net.minecraft.client.render.entity.PlayerEntityRenderer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * plan-dandao-path-v1 P3 fix -- 丹道异化贴图上身渲染注册（TPV）。
 *
 * <p>通过 Fabric {@link LivingEntityFeatureRendererRegistrationCallback} 把
 * {@link MutationFeatureRenderer} 挂到所有 {@link PlayerEntityRenderer}，
 * 严格参 {@code ArmorRenderBootstrap} / {@code WornPackRenderBootstrap}
 * （已验证可用的注册先例）。此前 {@link MutationFeatureRenderer} 从未注册，
 * 是身体异化叠加贴图永不显示的孤岛根因。
 */
public final class MutationRenderBootstrap {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-dandao-mutation-render");

    private MutationRenderBootstrap() {}

    public static void register() {
        LivingEntityFeatureRendererRegistrationCallback.EVENT.register(
            (entityType, entityRenderer, registrationHelper, context) -> {
                if (entityRenderer instanceof PlayerEntityRenderer playerRenderer) {
                    registrationHelper.register(new MutationFeatureRenderer<>(playerRenderer));
                }
            }
        );

        LOGGER.info("MutationRenderBootstrap: FeatureRenderer registered for dandao mutation overlay");
    }
}
