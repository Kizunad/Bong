package com.bong.client.mixin;

import com.bong.client.menu.MainMenuFlow;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(Screen.class)
public abstract class MixinMenuBackground {
    @Inject(method = "renderBackground(Lnet/minecraft/client/gui/DrawContext;)V", at = @At("HEAD"), cancellable = true)
    private void bong$menuBackdrop(DrawContext context, CallbackInfo ci) {
        if (MainMenuFlow.shouldRenderBackground()) {
            Screen screen = (Screen) (Object) this;
            MainMenuFlow.renderBackground(context, screen.width, screen.height);
            ci.cancel();
        }
    }
}
