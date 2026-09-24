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
    @Unique private ButtonWidget bong$retryButton;
    @Unique private ButtonWidget bong$backButton;

    @Inject(method = "init", at = @At("HEAD"), cancellable = true)
    private void bong$disconnectControls(CallbackInfo ci) {
        if (!MainMenuFlow.isConnectionParent(parent) && !MainMenuFlow.shouldRenderBackground()) {
            return;
        }
        MainMenuFlow.disconnected();
        Screen screen = (Screen) (Object) this;
        MinecraftClient client = MinecraftClient.getInstance();
        MenuScreenAccessor controls = (MenuScreenAccessor) screen;
        MainMenuReasonWidget reasonWidget = new MainMenuReasonWidget(reason, client.textRenderer, screen.width, screen.height);
        ButtonWidget retryButton = ButtonWidget.builder(Text.translatable("bong.menu.retry"), ignored -> MainMenuFlow.retry())
            .dimensions(0, 0, 96, 20).build();
        ButtonWidget backButton = ButtonWidget.builder(Text.translatable("bong.menu.back"), ignored -> client.setScreen(parent))
            .dimensions(0, 0, 96, 20).build();
        controls.bong$addMenuControl(reasonWidget);
        controls.bong$addMenuControl(retryButton);
        controls.bong$addMenuControl(backButton);
        bong$reasonWidget = reasonWidget;
        bong$retryButton = retryButton;
        bong$backButton = backButton;
        bong$positionButtons();
        ci.cancel();
    }

    @Inject(method = "initTabNavigation", at = @At("HEAD"), cancellable = true)
    private void bong$relayoutControls(CallbackInfo ci) {
        if (bong$reasonWidget == null) {
            return;
        }
        Screen screen = (Screen) (Object) this;
        bong$reasonWidget.relayout(screen.width, screen.height);
        bong$positionButtons();
        ci.cancel();
    }

    @Unique
    private void bong$positionButtons() {
        Screen screen = (Screen) (Object) this;
        int buttonY = bong$reasonWidget.getY() + bong$reasonWidget.getHeight() + 18;
        bong$retryButton.setX(screen.width / 2 - 101);
        bong$backButton.setX(screen.width / 2 + 5);
        bong$retryButton.setY(buttonY);
        bong$backButton.setY(buttonY);
    }

    @Inject(method = "render", at = @At("HEAD"), cancellable = true)
    private void bong$disconnectBackdrop(DrawContext context, int mouseX, int mouseY, float delta, CallbackInfo ci) {
        if (bong$reasonWidget != null) {
            MainMenuFlow.renderDisconnected(context, (Screen) (Object) this, bong$reasonWidget, mouseX, mouseY, delta);
            ci.cancel();
        }
    }
}
