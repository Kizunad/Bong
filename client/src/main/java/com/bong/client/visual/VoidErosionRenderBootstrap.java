package com.bong.client.visual;

import net.fabricmc.fabric.api.client.rendering.v1.LivingEntityFeatureRendererRegistrationCallback;
import net.minecraft.client.render.entity.PlayerEntityRenderer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * plan-combat-skill-feedback-bridges-v1 P3 fix — 注册 {@link VoidErosionModelAlphaRenderer}
 * 到全部 {@link PlayerEntityRenderer} 实例，保证玩家模型半透明渲染回路每帧被调用。
 *
 * <p>必须在 {@code BongClient.onInitializeClient()} 中调用 {@link #register()}。
 */
public final class VoidErosionRenderBootstrap {

    private static final Logger LOGGER = LoggerFactory.getLogger("bong/void_erosion/render_bootstrap");

    private VoidErosionRenderBootstrap() {}

    /**
     * 注册 {@link VoidErosionModelAlphaRenderer} FeatureRenderer。
     * 注册后每帧 {@link PlayerEntityRenderer} 渲染时会调用
     * {@link VoidErosionModelAlphaRenderer#render}，读取 {@link VoidErosionVisualStore}
     * 的 modelAlpha 并应用半透明。
     */
    public static void register() {
        LivingEntityFeatureRendererRegistrationCallback.EVENT.register(
                (entityType, entityRenderer, registrationHelper, context) -> {
                    if (entityRenderer instanceof PlayerEntityRenderer playerRenderer) {
                        registrationHelper.register(new VoidErosionModelAlphaRenderer(playerRenderer));
                    }
                }
        );
        LOGGER.debug("VoidErosionRenderBootstrap: ModelAlphaRenderer registered on PlayerEntityRenderer");
    }
}
