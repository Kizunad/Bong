package com.bong.client.mixin;

import com.bong.client.menu.MainMenuFlow;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.DisconnectedScreen;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.text.Text;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(DisconnectedScreen.class)
public abstract class MixinMenuDisconnected {
    @Shadow @Final private Screen parent;
    @Shadow @Final private Text reason;

    @Inject(method = "init", at = @At("TAIL"))
    private void bong$retryButton(CallbackInfo ci) {
        if (!MainMenuFlow.isConnectionParent(parent)) {
            return;
        }
        MainMenuFlow.disconnected();
        Screen screen = (Screen) (Object) this;
        ButtonWidget back = screen.children().stream().filter(ButtonWidget.class::isInstance)
            .map(ButtonWidget.class::cast).findFirst().orElse(null);
        if (back != null) {
            back.setWidth(96);
            back.setX(screen.width / 2 + 5);
            back.setMessage(Text.translatable("bong.menu.back"));
            ((MenuScreenAccessor) screen).bong$addMenuControl(
                ButtonWidget.builder(Text.translatable("bong.menu.retry"), ignored -> MainMenuFlow.retry())
                    .dimensions(screen.width / 2 - 101, back.getY(), 96, back.getHeight()).build());
        }
    }

    @Inject(method = "render", at = @At("HEAD"), cancellable = true)
    private void bong$disconnectBackdrop(DrawContext context, int mouseX, int mouseY, float delta, CallbackInfo ci) {
        if (MainMenuFlow.isConnectionParent(parent) || MainMenuFlow.shouldRenderBackground()) {
            MainMenuFlow.renderDisconnected(context, (Screen) (Object) this, reason, mouseX, mouseY);
            ci.cancel();
        }
    }
}
