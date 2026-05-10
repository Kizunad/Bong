package com.bong.client.mixin;

import com.bong.client.ui.ScreenTransitionOverlay;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(Screen.class)
public class MixinScreenTransitionRender {
    @Shadow public int width;
    @Shadow public int height;

    @Inject(method = "renderWithTooltip", at = @At("RETURN"))
    private void bong$renderTransitionOverlay(DrawContext context, int mouseX, int mouseY, float delta, CallbackInfo ci) {
        ScreenTransitionOverlay.render(context, width, height, System.currentTimeMillis());
    }
}
