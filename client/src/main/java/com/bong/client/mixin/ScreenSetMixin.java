package com.bong.client.mixin;

import com.bong.client.ui.ScreenTransitionController;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.screen.ingame.InventoryScreen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(MinecraftClient.class)
public class ScreenSetMixin {
    @Inject(method = "setScreen", at = @At("HEAD"), cancellable = true)
    private void bong$playScreenTransition(Screen screen, CallbackInfo ci) {
        if (screen instanceof InventoryScreen) {
            return;
        }
        if (ScreenTransitionController.interceptSetScreen((MinecraftClient) (Object) this, screen)) {
            ci.cancel();
        }
    }
}
