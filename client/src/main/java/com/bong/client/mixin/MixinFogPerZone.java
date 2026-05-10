package com.bong.client.mixin;

import com.bong.client.environment.EnvironmentFogController;
import net.minecraft.client.render.BackgroundRenderer;
import net.minecraft.client.render.Camera;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(BackgroundRenderer.class)
public class MixinFogPerZone {
    @Inject(
        method = "applyFog(Lnet/minecraft/client/render/Camera;Lnet/minecraft/client/render/BackgroundRenderer$FogType;FZF)V",
        at = @At("TAIL")
    )
    private static void bong$applyZoneEnvironmentFog(
        Camera camera,
        BackgroundRenderer.FogType fogType,
        float viewDistance,
        boolean thickFog,
        float tickDelta,
        CallbackInfo ci
    ) {
        if (fogType != BackgroundRenderer.FogType.FOG_TERRAIN || thickFog) {
            return;
        }
        EnvironmentFogController.applyFog();
    }
}
