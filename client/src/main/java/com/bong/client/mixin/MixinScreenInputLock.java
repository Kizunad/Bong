package com.bong.client.mixin;

import com.bong.client.ui.ScreenTransitionController;
import net.minecraft.client.gui.screen.Screen;
import org.lwjgl.glfw.GLFW;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(Screen.class)
public class MixinScreenInputLock {
    @Inject(method = "keyPressed(III)Z", at = @At("HEAD"), cancellable = true)
    private void bong$lockKeysDuringTransition(int keyCode, int scanCode, int modifiers, CallbackInfoReturnable<Boolean> cir) {
        if (!ScreenTransitionController.inputLocked()) {
            return;
        }
        if (keyCode == GLFW.GLFW_KEY_ESCAPE) {
            ScreenTransitionController.cancelAndClose(net.minecraft.client.MinecraftClient.getInstance());
        }
        cir.setReturnValue(true);
    }
}
