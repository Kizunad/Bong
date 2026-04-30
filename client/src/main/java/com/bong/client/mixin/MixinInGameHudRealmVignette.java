package com.bong.client.mixin;

import com.bong.client.visual.realm_vision.RealmVisionVignetteOverlay;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.hud.InGameHud;
import net.minecraft.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(InGameHud.class)
public class MixinInGameHudRealmVignette {
    @Inject(method = "renderVignetteOverlay", at = @At("TAIL"))
    private void bong$renderRealmVisionVignette(DrawContext context, Entity entity, CallbackInfo ci) {
        RealmVisionVignetteOverlay.render(
            context,
            context.getScaledWindowWidth(),
            context.getScaledWindowHeight(),
            System.currentTimeMillis() / 50L
        );
    }
}
