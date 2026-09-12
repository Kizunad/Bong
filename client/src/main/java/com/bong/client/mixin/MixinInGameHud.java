package com.bong.client.mixin;

import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.hud.InGameHud;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * 关闭原生快捷栏、生存状态条、准星和攻击冷却提示。
 *
 * <p>Bong 的武器槽、生命/伤口、状态与进度 HUD 已由自有 planner 统一接管，
 * 准星入口包含中央剑形攻击冷却提示，快捷栏入口包含另一种攻击指示位置。
 */
@Mixin(InGameHud.class)
public class MixinInGameHud {

    @Inject(method = "renderCrosshair", at = @At("HEAD"), cancellable = true)
    private void bong$hideCrosshair(DrawContext context, CallbackInfo ci) {
        ci.cancel();
    }

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
