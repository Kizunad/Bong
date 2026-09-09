package com.bong.client.mixin;

import com.bong.client.menu.MainMenuFlow;
import com.bong.client.menu.MainMenuReasonWidget;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.DisconnectedScreen;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.text.Text;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(DisconnectedScreen.class)
public abstract class MixinMenuDisconnected {
    @Shadow @Final private Screen parent;
    @Shadow @Final private Text reason;
    @Unique private MainMenuReasonWidget bong$reasonWidget;

    @Inject(method = "init", at = @At("HEAD"), cancellable = true)
    private void bong$disconnectControls(CallbackInfo ci) {
        if (!MainMenuFlow.isConnectionParent(parent) && !MainMenuFlow.shouldRenderBackground()) {
            return;
        }
        MainMenuFlow.disconnected();
        Screen screen = (Screen) (Object) this;
        MinecraftClient client = MinecraftClient.getInstance();
        MenuScreenAccessor controls = (MenuScreenAccessor) screen;
        bong$reasonWidget = controls.bong$addMenuControl(
            new MainMenuReasonWidget(reason, client.textRenderer, screen.width, screen.height));
        int buttonY = bong$reasonWidget.getY() + bong$reasonWidget.getHeight() + 18;
        controls.bong$addMenuControl(ButtonWidget.builder(Text.translatable("bong.menu.retry"), ignored -> MainMenuFlow.retry())
            .dimensions(screen.width / 2 - 101, buttonY, 96, 20).build());
        controls.bong$addMenuControl(ButtonWidget.builder(Text.translatable("bong.menu.back"), ignored -> client.setScreen(parent))
            .dimensions(screen.width / 2 + 5, buttonY, 96, 20).build());
        ci.cancel();
    }

    @Inject(method = "render", at = @At("HEAD"), cancellable = true)
    private void bong$disconnectBackdrop(DrawContext context, int mouseX, int mouseY, float delta, CallbackInfo ci) {
        if (bong$reasonWidget != null) {
            MainMenuFlow.renderDisconnected(context, (Screen) (Object) this, bong$reasonWidget, mouseX, mouseY, delta);
            ci.cancel();
        }
    }
}
