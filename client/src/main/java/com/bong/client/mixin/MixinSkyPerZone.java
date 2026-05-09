package com.bong.client.mixin;

import com.bong.client.environment.EnvironmentSkyController;
import net.minecraft.client.render.Camera;
import net.minecraft.client.render.WorldRenderer;
import net.minecraft.client.util.math.MatrixStack;
import org.joml.Matrix4f;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(WorldRenderer.class)
public class MixinSkyPerZone {
    @Inject(
        method = "renderSky(Lnet/minecraft/client/util/math/MatrixStack;Lorg/joml/Matrix4f;FLnet/minecraft/client/render/Camera;ZLjava/lang/Runnable;)V",
        at = @At("HEAD")
    )
    private void bong$applyZoneEnvironmentSkyTint(
        MatrixStack matrices,
        Matrix4f projectionMatrix,
        float tickDelta,
        Camera camera,
        boolean thickFog,
        Runnable fogCallback,
        CallbackInfo ci
    ) {
        EnvironmentSkyController.applyBeforeSky();
    }

    @Inject(
        method = "renderSky(Lnet/minecraft/client/util/math/MatrixStack;Lorg/joml/Matrix4f;FLnet/minecraft/client/render/Camera;ZLjava/lang/Runnable;)V",
        at = @At("TAIL")
    )
    private void bong$resetZoneEnvironmentSkyTint(
        MatrixStack matrices,
        Matrix4f projectionMatrix,
        float tickDelta,
        Camera camera,
        boolean thickFog,
        Runnable fogCallback,
        CallbackInfo ci
    ) {
        EnvironmentSkyController.resetAfterSky();
    }
}
