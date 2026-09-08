package com.bong.client.mixin;

import com.bong.client.menu.MainMenuScreen;
import com.bong.client.menu.MainMenuFlow;
import com.bong.client.ui.adapter.owo.OwoXmlTemplateRegistry;
import io.wispforest.owo.ui.parsing.UIModelLoader;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.screen.TitleScreen;
import net.minecraft.client.gui.screen.multiplayer.MultiplayerScreen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(MinecraftClient.class)
public abstract class MixinTitleMenu {
    @Inject(method = "setScreen", at = @At("HEAD"), cancellable = true)
    private void bong$mainMenu(Screen screen, CallbackInfo ci) {
        MinecraftClient client = (MinecraftClient) (Object) this;
        if (screen == null && client.world != null) {
            MainMenuFlow.enteredWorld();
        }
        if ((screen instanceof TitleScreen || screen instanceof MultiplayerScreen || (screen == null && client.world == null))
            && bong$menuResourcesReady(client)) {
            client.setScreen(new MainMenuScreen());
            ci.cancel();
        }
    }

    @Inject(method = "tick", at = @At("HEAD"))
    private void bong$openAfterResourceLoad(CallbackInfo ci) {
        MinecraftClient client = (MinecraftClient) (Object) this;
        if ((client.currentScreen instanceof TitleScreen || client.currentScreen instanceof MultiplayerScreen)
            && bong$menuResourcesReady(client)) {
            client.setScreen(new MainMenuScreen());
        }
    }

    @Unique
    private static boolean bong$menuResourcesReady(MinecraftClient client) {
        return client.getOverlay() == null
            && UIModelLoader.get(OwoXmlTemplateRegistry.production().identifierFor("main-menu")) != null;
    }
}
