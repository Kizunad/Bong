package com.bong.client.fauna;

import net.minecraft.client.render.OverlayTexture;
import net.minecraft.client.render.RenderLayer;
import net.minecraft.client.render.VertexConsumer;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.util.Identifier;
import software.bernie.geckolib.cache.object.BakedGeoModel;
import software.bernie.geckolib.renderer.GeoRenderer;
import software.bernie.geckolib.renderer.layer.GeoRenderLayer;

/**
 * plan-devour-rat-model P2 —— fauna 实体 emissive 发光层。
 *
 * <p>在实体本体渲染完之后，用 {@code <底图>_glow.png} 把整个模型再画一遍，走
 * {@link RenderLayer#getEntityTranslucentEmissive}（lightmap 禁用 = 全亮，alpha 生效
 * = 透明像素不画）。因此 glow 贴图里**只有要发光的像素是非透明的**：噬元鼠是红眼
 * （恒亮）+ 尾脊蓝格（按吸元档位 q0/q1/q2 递增 0 / 2 / 4 段）。
 *
 * <p>为什么不用 GeckoLib 自带的 {@code AutoGlowingGeoLayer}：它按
 * {@code _glowmask} 后缀取图，且语义是"掩码"——从**底图**里挖出被 mask 命中的像素挪到
 * 发光层、并把底图对应像素清成透明。本仓库的 glow 贴图是直接给颜色的叠加层，语义不同；
 * 自己实现也省掉了库对底图 {@code NativeImage} 的就地改写。
 *
 * <p>挂载由 {@link FaunaRenderer} 按 {@link FaunaVisualKind#hasEmissiveGlow()} 决定——
 * 没备 glow 资产的物种绝不挂，否则会渲染 missing texture（紫黑格）覆盖整只怪。
 *
 * <p>开销：{@code reRender} 会把整个模型的顶点再走一遍（噬元鼠 93 cube → ×2），与
 * GeckoLib 自带 {@code AutoGlowingGeoLayer} 同一量级；透明像素不产生 fragment，故填充
 * 开销很小，代价主要在顶点提交。鼠患成群时这是已知的固定倍数开销，非按档位递增。
 */
public final class FaunaEmissiveGlowLayer extends GeoRenderLayer<FaunaEntity> {

    /**
     * 全亮 packed light —— 等价于 {@code LightmapTextureManager.pack(15, 15)}
     * （block=15 装进低 4 位组、sky=15 装进高位组）= 15728880。
     *
     * <p>{@link RenderLayer#getEntityTranslucentEmissive} 本身已禁用 lightmap（发光层恒全亮），
     * 这个值只是"就算将来换成会读 lightmap 的 RenderLayer 也不会被环境光压暗"的保险。
     */
    public static final int FULL_BRIGHT_LIGHT = (15 << 20) | (15 << 4);

    public FaunaEmissiveGlowLayer(GeoRenderer<FaunaEntity> renderer) {
        super(renderer);
    }

    @Override
    public void render(
        MatrixStack poseStack,
        FaunaEntity animatable,
        BakedGeoModel bakedModel,
        RenderLayer renderType,
        VertexConsumerProvider bufferSource,
        VertexConsumer buffer,
        float partialTick,
        int packedLight,
        int packedOverlay
    ) {
        // 二次防御：renderer 构造期已按 kind 决定挂不挂层，这里再挡一次，防止将来
        // 有人把本层挂到没有 glow 资产的物种上而静默渲染紫黑格。
        if (!animatable.visualKind().hasEmissiveGlow()) {
            return;
        }
        // 逐帧从 renderer 取当前底图 → 推导 glow 贴图：噬元鼠换档（q0→q1→q2）时
        // 发光层必须跟着换，不能在构造期把贴图缓存死。
        Identifier glowTexture = FaunaModel.glowTextureFor(getTextureResource(animatable));
        RenderLayer emissive = RenderLayer.getEntityTranslucentEmissive(glowTexture);
        getRenderer().reRender(
            bakedModel,
            poseStack,
            bufferSource,
            animatable,
            emissive,
            bufferSource.getBuffer(emissive),
            partialTick,
            FULL_BRIGHT_LIGHT,
            OverlayTexture.DEFAULT_UV,
            1.0f,
            1.0f,
            1.0f,
            1.0f
        );
    }
}
