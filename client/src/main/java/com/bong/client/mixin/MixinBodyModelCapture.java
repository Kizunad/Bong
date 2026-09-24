package com.bong.client.mixin;

import com.bong.client.ui.model.BodyModelCapture;
import net.minecraft.client.render.VertexConsumer;
import net.minecraft.client.model.ModelPart;
import net.minecraft.client.util.math.MatrixStack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** 捕获实际部件绘制入口，包含动画库施加的上半身附加变换。 */
@Mixin(ModelPart.class)
public abstract class MixinBodyModelCapture {
    @Inject(method = "render(Lnet/minecraft/client/util/math/MatrixStack;Lnet/minecraft/client/render/VertexConsumer;IIFFFF)V", at = @At("HEAD"))
    private void bong$captureBody(MatrixStack matrices, VertexConsumer vertices, int light, int overlay,
                                  float red, float green, float blue, float alpha, CallbackInfo ci) {
        BodyModelCapture.onPartRender((ModelPart) (Object) this, matrices);
    }
}
