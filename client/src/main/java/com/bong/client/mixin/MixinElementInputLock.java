package com.bong.client.mixin;

import com.bong.client.ui.ScreenTransitionController;
import net.minecraft.client.gui.Element;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(Element.class)
public interface MixinElementInputLock {
    @Inject(method = "charTyped(CI)Z", at = @At("HEAD"), cancellable = true)
    private void bong$lockCharsDuringTransition(char chr, int modifiers, CallbackInfoReturnable<Boolean> cir) {
        if (ScreenTransitionController.inputLocked()) {
            cir.setReturnValue(true);
        }
    }

    @Inject(method = "mouseClicked(DDI)Z", at = @At("HEAD"), cancellable = true)
    private void bong$lockMouseClickDuringTransition(double mouseX, double mouseY, int button, CallbackInfoReturnable<Boolean> cir) {
        if (ScreenTransitionController.inputLocked()) {
            cir.setReturnValue(true);
        }
    }

    @Inject(method = "mouseReleased(DDI)Z", at = @At("HEAD"), cancellable = true)
    private void bong$lockMouseReleaseDuringTransition(double mouseX, double mouseY, int button, CallbackInfoReturnable<Boolean> cir) {
        if (ScreenTransitionController.inputLocked()) {
            cir.setReturnValue(true);
        }
    }

    @Inject(method = "mouseScrolled(DDD)Z", at = @At("HEAD"), cancellable = true)
    private void bong$lockMouseScrollDuringTransition(double mouseX, double mouseY, double amount, CallbackInfoReturnable<Boolean> cir) {
        if (ScreenTransitionController.inputLocked()) {
            cir.setReturnValue(true);
        }
    }
}
