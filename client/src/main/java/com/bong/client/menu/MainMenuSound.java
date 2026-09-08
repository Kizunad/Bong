package com.bong.client.menu;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.sound.MovingSoundInstance;
import net.minecraft.client.sound.SoundInstance;
import net.minecraft.sound.SoundCategory;
import net.minecraft.sound.SoundEvents;

/** 使用原版随包环境声；菜单没有 ClientWorld，声音相对于听者播放。 */
final class MainMenuSound extends MovingSoundInstance {
    private static MainMenuSound current;
    private static boolean enabled;

    private MainMenuSound() {
        super(SoundEvents.AMBIENT_SOUL_SAND_VALLEY_LOOP.value(), SoundCategory.AMBIENT, SoundInstance.createRandom());
        repeat = true;
        repeatDelay = 0;
        relative = true;
        attenuationType = AttenuationType.NONE;
        volume = 0.001f;
        pitch = 0.8f;
    }

    static void configure(boolean value) {
        enabled = value;
        if (value && (current == null || current.isDone()
            || !MinecraftClient.getInstance().getSoundManager().isPlaying(current))) {
            current = new MainMenuSound();
            MinecraftClient.getInstance().getSoundManager().play(current);
        }
    }

    @Override
    public void tick() {
        MinecraftClient client = MinecraftClient.getInstance();
        float target = enabled && MainMenuFlow.shouldRenderBackground() && client.currentScreen != null ? 0.12f : 0;
        volume += (target - volume) * 0.14f;
        if (target == 0 && volume < 0.001f) {
            setDone();
        }
    }
}
