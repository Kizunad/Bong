package com.bong.client.mixin;

import com.bong.client.menu.MainMenuFlow;
import net.minecraft.client.sound.MusicTracker;
import net.minecraft.client.sound.SoundInstance;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(MusicTracker.class)
public abstract class MixinMenuMusic {
    @Shadow private SoundInstance current;
    @Shadow public abstract void stop();

    @Inject(method = "tick", at = @At("HEAD"), cancellable = true)
    private void bong$menuAmbienceOnly(CallbackInfo ci) {
        if (MainMenuFlow.shouldRenderBackground()) {
            if (current != null) stop();
            ci.cancel();
        }
    }
}
