package com.bong.client.mixin;

import com.bong.client.ui.ScreenTransitionController;
import com.bong.client.menu.MainMenuFlow;
import com.bong.client.menu.MainMenuScreen;
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
        if (screen instanceof MainMenuScreen || MainMenuFlow.shouldRenderBackground()) {
            // 登录前由分层场景持有动画，不能延迟原版连接和取消的切屏。
            ScreenTransitionController.clearOnDisconnect();
            return;
        }
        if (screen instanceof InventoryScreen) {
            // Vanilla inventory is rerouted by MixinMinecraftClient to Bong InspectScreen.
            // The actual backpack transition is therefore attached to InspectScreen.
            return;
        }
        if (ScreenTransitionController.interceptSetScreen((MinecraftClient) (Object) this, screen)) {
            ci.cancel();
        }
    }
}
