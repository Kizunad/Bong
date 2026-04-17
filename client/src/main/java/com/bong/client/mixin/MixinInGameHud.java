package com.bong.client.mixin;

import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.hud.InGameHud;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * plan-weapon-v1 §4.2：关闭原生底部 hotbar 渲染。
 *
 * <p>Bong 的武器槽 + 快捷栏由 {@code BongHotbarHudPlanner}（W4）统一接管，
 * 不再复用 vanilla 底部 9 格样式。此处直接 HEAD cancel 最稳妥——不影响 F3 调试、
 * 聊天气泡、准星、十字准线等其他 HUD 绘制路径（这些都在 {@code InGameHud.render}
 * 的其他子调用里，不通过 {@code renderHotbar}）。
 */
@Mixin(InGameHud.class)
public class MixinInGameHud {

    @Inject(method = "renderHotbar", at = @At("HEAD"), cancellable = true)
    private void bong$replaceHotbar(float tickDelta, DrawContext context, CallbackInfo ci) {
        ci.cancel();
    }
}
