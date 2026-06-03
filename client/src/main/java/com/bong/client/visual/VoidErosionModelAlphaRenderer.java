package com.bong.client.visual;

import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.client.render.OverlayTexture;
import net.minecraft.client.render.RenderLayer;
import net.minecraft.client.render.VertexConsumer;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.render.entity.feature.FeatureRenderer;
import net.minecraft.client.render.entity.feature.FeatureRendererContext;
import net.minecraft.client.render.entity.model.PlayerEntityModel;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.util.Identifier;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * plan-combat-skill-feedback-bridges-v1 P3 fix — 每帧读 {@link VoidErosionVisualStore}
 * 中的 {@code modelAlpha} 并以半透明方式重绘玩家模型，实现虚蚀阶段 4 = 0.4 透明度渐变。
 *
 * <p>接入方式：{@link VoidErosionRenderBootstrap} 通过
 * {@code LivingEntityFeatureRendererRegistrationCallback} 在所有
 * {@link PlayerEntityRenderer} 上注册本 renderer，保证每帧调用。
 *
 * <p>当 {@code modelAlpha >= 1.0f}（无虚蚀或阶段 0）时跳过渲染，性能零开销。
 * 阶段 1+ 时，以玩家皮肤纹理 + {@code RenderLayer.getEntityTranslucent}
 * 叠加一层按 modelAlpha 缩减的半透明版本，视觉上呈现玩家渐变透明效果。
 */
public final class VoidErosionModelAlphaRenderer
        extends FeatureRenderer<AbstractClientPlayerEntity, PlayerEntityModel<AbstractClientPlayerEntity>> {

    private static final Logger LOGGER = LoggerFactory.getLogger("bong/void_erosion/alpha_renderer");

    /** 低于此值才启用半透明渲染（1.0 = 完全不透明，不需要渲染）。 */
    private static final float ALPHA_THRESHOLD = 0.999f;

    public VoidErosionModelAlphaRenderer(
            FeatureRendererContext<AbstractClientPlayerEntity,
                    PlayerEntityModel<AbstractClientPlayerEntity>> context) {
        super(context);
    }

    /**
     * 每帧由 {@link PlayerEntityRenderer} 的 feature 渲染循环调用。
     * 查询 {@link VoidErosionVisualStore} 获取当前 modelAlpha，若小于 1.0 则绘制半透明玩家模型。
     */
    @Override
    public void render(
            MatrixStack matrices,
            VertexConsumerProvider vertexConsumers,
            int light,
            AbstractClientPlayerEntity entity,
            float limbAngle,
            float limbDistance,
            float tickDelta,
            float animationProgress,
            float headYaw,
            float headPitch
    ) {
        VoidErosionVisualStore.State state = VoidErosionVisualStore.snapshot();
        if (state == null || state.modelAlpha() >= ALPHA_THRESHOLD) {
            // 无虚蚀或阶段 0 / 未收到 payload — 不渲染
            return;
        }

        float alpha = Math.max(0.0f, Math.min(1.0f, state.modelAlpha()));
        LOGGER.trace("void_erosion_alpha: entity={} stage={} alpha={}",
                entity.getEntityName(), state.stage(), alpha);

        // 使用玩家皮肤纹理 + EntityTranslucent 渲染半透明叠加层
        // MC 1.20.1 yarn API: getSkinTexture() 直接返回 Identifier
        Identifier skinTexture = entity.getSkinTexture();
        RenderLayer renderLayer = RenderLayer.getEntityTranslucent(skinTexture);
        VertexConsumer buffer = vertexConsumers.getBuffer(renderLayer);

        // 轻微放大（1.002f）防止 z-fighting（与原始皮肤层重叠）
        matrices.push();
        matrices.scale(1.002f, 1.002f, 1.002f);
        // RGBA: r=1, g=1, b=1, a=alpha（lerp 到目标透明度）
        this.getContextModel().render(
                matrices, buffer, light, OverlayTexture.DEFAULT_UV,
                1.0f, 1.0f, 1.0f, alpha);
        matrices.pop();
    }
}
