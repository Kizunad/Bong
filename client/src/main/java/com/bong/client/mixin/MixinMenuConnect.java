package com.bong.client.mixin;

import com.bong.client.menu.MainMenuFlow;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.ConnectScreen;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.text.Text;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(ConnectScreen.class)
public abstract class MixinMenuConnect {
    @Shadow @Final private Screen parent;
    @Shadow private Text status;

    // 原版先播报连接状态，再接管文字和按钮绘制。
    @Inject(method = "render", at = @At(value = "INVOKE", target =
        "Lnet/minecraft/client/gui/DrawContext;drawCenteredTextWithShadow(Lnet/minecraft/client/font/TextRenderer;Lnet/minecraft/text/Text;III)V"), cancellable = true)
    private void bong$connectBackdrop(DrawContext context, int mouseX, int mouseY, float delta, CallbackInfo ci) {
        if (MainMenuFlow.isConnectionParent(parent)) {
            MainMenuFlow.renderConnection(context, (Screen) (Object) this, status, mouseX, mouseY);
            ci.cancel();
        }
    }
}
