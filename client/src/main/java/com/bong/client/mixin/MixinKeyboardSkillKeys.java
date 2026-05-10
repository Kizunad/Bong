package com.bong.client.mixin;

import com.bong.client.combat.SkillBarKeyRouter;
import com.bong.client.ui.ScreenTransitionController;
import com.bong.client.ui.TransitionInputPolicy;
import net.minecraft.client.Keyboard;
import net.minecraft.client.MinecraftClient;
import org.lwjgl.glfw.GLFW;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(Keyboard.class)
public class MixinKeyboardSkillKeys {
    @Inject(method = "onKey", at = @At("HEAD"), cancellable = true)
    private void bong$skillBarHotbarKeys(long window, int key, int scancode, int action, int modifiers, CallbackInfo ci) {
        TransitionInputPolicy.KeyDecision transitionDecision =
            TransitionInputPolicy.keyDecision(ScreenTransitionController.inputLocked(), key, action);
        if (transitionDecision == TransitionInputPolicy.KeyDecision.CANCEL_AND_CLOSE) {
            ScreenTransitionController.cancelAndClose(MinecraftClient.getInstance());
            ci.cancel();
            return;
        }
        if (transitionDecision == TransitionInputPolicy.KeyDecision.CONSUME) {
            ci.cancel();
            return;
        }

        if (action != GLFW.GLFW_PRESS) {
            return;
        }
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null || client.player == null || client.currentScreen != null) {
            return;
        }
        if (key == GLFW.GLFW_KEY_F && SkillBarKeyRouter.shouldCancelAnqiContainerKey()) {
            ci.cancel();
            return;
        }
        int slot = key - GLFW.GLFW_KEY_1;
        if (slot < 0 || slot >= 9) {
            return;
        }
        if (SkillBarKeyRouter.shouldCancelHotbarKey(slot)) {
            ci.cancel();
        }
    }
}
