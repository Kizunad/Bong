package com.bong.client.mixin;

import com.bong.client.botany.BotanyDragState;
import com.bong.client.botany.HarvestSessionStore;
import com.bong.client.botany.HarvestSessionViewModel;
import com.bong.client.ui.ScreenTransitionController;
import com.bong.client.ui.TransitionInputPolicy;
import net.minecraft.client.Mouse;
import net.minecraft.client.MinecraftClient;
import org.lwjgl.glfw.GLFW;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(Mouse.class)
public class MixinMouse {

    @Inject(
        method = "updateMouse",
        at = @At("HEAD"),
        cancellable = true
    )
    private void bong$guardNullPlayerOnUpdate(CallbackInfo ci) {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null || client.player == null) {
            ci.cancel();
        }
    }

    @Inject(
        method = "onMouseButton(JIII)V",
        at = @At("HEAD"),
        cancellable = true
    )
    private void bong$captureHarvestPanelDrag(long window, int button, int action, int mods, CallbackInfo ci) {
        if (TransitionInputPolicy.shouldBlockMouse(ScreenTransitionController.inputLocked(), action)) {
            ci.cancel();
            return;
        }
        if (button != GLFW.GLFW_MOUSE_BUTTON_LEFT) {
            return;
        }
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null || client.currentScreen != null) {
            return;
        }
        HarvestSessionViewModel session = HarvestSessionStore.snapshot();
        if (!session.interactive()) {
            return;
        }
        double mx = client.mouse.getX() * client.getWindow().getScaledWidth()
            / (double) client.getWindow().getWidth();
        double my = client.mouse.getY() * client.getWindow().getScaledHeight()
            / (double) client.getWindow().getHeight();
        int translatedAction = action == GLFW.GLFW_PRESS ? 1 : action == GLFW.GLFW_RELEASE ? 0 : -1;
        if (translatedAction < 0) {
            return;
        }
        if (BotanyDragState.onLeftButton(translatedAction, mx, my)) {
            ci.cancel();
        }
    }
}
