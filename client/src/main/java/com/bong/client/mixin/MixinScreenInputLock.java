package com.bong.client.mixin;

import com.bong.client.ui.ScreenTransitionController;
import com.bong.client.ui.TransitionInputPolicy;
import net.minecraft.client.gui.screen.Screen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(Screen.class)
public class MixinScreenInputLock {
    @Inject(method = "keyPressed(III)Z", at = @At("HEAD"), cancellable = true)
    private void bong$lockKeysDuringTransition(int keyCode, int scanCode, int modifiers, CallbackInfoReturnable<Boolean> cir) {
        TransitionInputPolicy.KeyDecision decision =
            TransitionInputPolicy.keyDecision(ScreenTransitionController.inputLocked(), keyCode, org.lwjgl.glfw.GLFW.GLFW_PRESS);
        if (decision == TransitionInputPolicy.KeyDecision.CANCEL_AND_CLOSE) {
            ScreenTransitionController.cancelAndClose(net.minecraft.client.MinecraftClient.getInstance());
            cir.setReturnValue(true);
            return;
        }
        if (decision == TransitionInputPolicy.KeyDecision.CONSUME) {
            cir.setReturnValue(true);
        }
    }
}
