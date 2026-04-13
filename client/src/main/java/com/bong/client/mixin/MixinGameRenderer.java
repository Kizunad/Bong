package com.bong.client.mixin;

import com.bong.client.hud.BongHudStateStore;
import com.bong.client.state.VisualEffectState;
import com.bong.client.visual.CameraFovOffset;
import net.minecraft.client.render.Camera;
import net.minecraft.client.render.GameRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * 为 FOV_ZOOM_IN / FOV_STRETCH 视觉效果叠加 FOV 偏移。
 *
 * <p>注入点：{@link GameRenderer#getFov} 的 RETURN——所有 MC 调用链（世界渲染、水下 FOV 动画、sprint
 * 缩放等）最终都通过这个方法获取 FOV，这里是唯一需要改动的地方。
 *
 * <p><b>与 vanilla/其他 FOV 修饰并存</b>：我们只做**叠加**，不覆盖返回值。水下/跑步/
 * 缩放等 MC 原生 FOV 动画继续生效；本 Mixin 只在现有值上加正/负偏移。
 *
 * <p>非 FOV 类 state 时 {@link CameraFovOffset#compute} 返回 0，注入等价于直通。
 */
@Mixin(GameRenderer.class)
public class MixinGameRenderer {

    @Inject(
        method = "getFov(Lnet/minecraft/client/render/Camera;FZ)D",
        at = @At("RETURN"),
        cancellable = true
    )
    private void bong$applyFovOffset(
        Camera camera,
        float tickDelta,
        boolean changingFov,
        CallbackInfoReturnable<Double> cir
    ) {
        VisualEffectState state = BongHudStateStore.snapshot().visualEffectState();
        double delta = CameraFovOffset.compute(state, System.currentTimeMillis());
        if (delta == 0.0) {
            return;
        }
        double current = cir.getReturnValueD();
        cir.setReturnValue(current + delta);
    }
}
