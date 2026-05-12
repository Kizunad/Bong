package com.bong.client.mixin;

import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.hud.InGameHud;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * plan-weapon-v1 §4.2 + plan-survival-gate-v1 P1：关闭原生底部 hotbar 与生存状态条渲染。
 *
 * <p>Bong 的武器槽、生命/伤口、状态与进度 HUD 已由自有 planner 统一接管，
 * 不再复用 vanilla 底部 9 格、红心、饥饿、护甲、氧气和经验条样式。
 * 此处直接 HEAD cancel 最稳妥——不影响 F3 调试、聊天气泡、准星、十字准线等
 * 其他 HUD 绘制路径。
 */
@Mixin(InGameHud.class)
public class MixinInGameHud {

    @Inject(method = "renderHotbar", at = @At("HEAD"), cancellable = true)
    private void bong$replaceHotbar(float tickDelta, DrawContext context, CallbackInfo ci) {
        ci.cancel();
    }

    @Inject(method = "renderStatusBars", at = @At("HEAD"), cancellable = true)
    private void bong$hideStatusBars(DrawContext context, CallbackInfo ci) {
        ci.cancel();
    }

    @Inject(method = "renderExperienceBar", at = @At("HEAD"), cancellable = true)
    private void bong$hideExperienceBar(DrawContext context, int x, CallbackInfo ci) {
        ci.cancel();
    }
}
