package com.bong.client.mixin;

import com.bong.client.menu.MainMenuFlow;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.DownloadingTerrainScreen;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.text.Text;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(DownloadingTerrainScreen.class)
public abstract class MixinMenuTerrain {
    @Inject(method = "render", at = @At("HEAD"), cancellable = true)
    private void bong$terrainBackdrop(DrawContext context, int mouseX, int mouseY, float delta, CallbackInfo ci) {
        if (MainMenuFlow.isConnecting()) {
            MainMenuFlow.renderConnection(context, (Screen) (Object) this,
                Text.translatable("multiplayer.downloadingTerrain"), mouseX, mouseY);
            ci.cancel();
        }
    }
}
