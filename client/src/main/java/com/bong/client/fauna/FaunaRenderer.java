package com.bong.client.fauna;

import net.minecraft.client.render.entity.EntityRendererFactory;
import net.minecraft.client.util.math.MatrixStack;
import software.bernie.geckolib.renderer.GeoEntityRenderer;

public final class FaunaRenderer extends GeoEntityRenderer<FaunaEntity> {
    public FaunaRenderer(EntityRendererFactory.Context ctx, FaunaVisualKind visualKind) {
        super(ctx, new FaunaModel());
        this.withScale(visualKind.renderScale());
        this.shadowRadius = visualKind.shadowRadius();
        // plan-devour-rat-model P2 —— 只给备齐 `<底图>_glow.png` 的物种挂 emissive 层；
        // 缺资产的物种挂了会渲染 missing texture（紫黑格）盖住整只怪。
        if (visualKind.hasEmissiveGlow()) {
            this.addRenderLayer(new FaunaEmissiveGlowLayer(this));
        }
    }

    /**
     * marker 实体不下发 yaw → 服务端传入的 {@code rotationYaw} 恒为 0、模型不跟移动（"倒着跑"）。
     * 对 {@link FaunaVisualKind#facesMovementDirection()} 的物种改用客户端按位移自算的
     * {@link FaunaEntity#movementYaw()} 驱动朝向；其余物种沿用引擎 yaw。
     *
     * <p>GeckoLib 在 {@code applyRotations} 里以 {@code 180° − rotationYaw} 绕 Y 旋转模型，
     * 故这里换掉 rotationYaw 即等价于旋转整只模型的朝向；不影响动画（动画在骨骼局部空间叠加）。
     */
    @Override
    protected void applyRotations(
        FaunaEntity animatable,
        MatrixStack poseStack,
        float ageInTicks,
        float rotationYaw,
        float partialTick
    ) {
        float baseYaw = animatable.visualKind().facesMovementDirection()
            ? animatable.movementYaw()
            : rotationYaw;
        super.applyRotations(animatable, poseStack, ageInTicks, baseYaw, partialTick);
    }
}
