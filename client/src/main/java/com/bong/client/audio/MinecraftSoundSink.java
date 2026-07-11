package com.bong.client.audio;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.sound.SoundInstance;
import net.minecraft.sound.SoundCategory;
import net.minecraft.util.math.random.Random;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

public final class MinecraftSoundSink implements SoundSink {
    private final Map<Long, List<FadeableSoundInstance>> activeByInstance = new ConcurrentHashMap<>();

    @Override
    public boolean play(AudioScheduledSound sound) {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null || client.getSoundManager() == null) {
            return false;
        }

        boolean relative = sound.attenuation() == AudioAttenuation.PLAYER_LOCAL
            || sound.attenuation() == AudioAttenuation.SELF;
        SoundInstance.AttenuationType attenuationType = relative
            ? SoundInstance.AttenuationType.NONE
            : SoundInstance.AttenuationType.LINEAR;
        AudioPosition pos = sound.pos().orElseGet(() -> {
            if (client.player != null) {
                return new AudioPosition(
                    (int) Math.floor(client.player.getX()),
                    (int) Math.floor(client.player.getY()),
                    (int) Math.floor(client.player.getZ())
                );
            }
            return new AudioPosition(0, 0, 0);
        });

        FadeableSoundInstance instance = new FadeableSoundInstance(
            sound.sound(),
            toMinecraftCategory(sound.category()),
            sound.volume(),
            sound.pitch(),
            Random.create(),
            false,
            0,
            attenuationType,
            pos.x(),
            pos.y(),
            pos.z(),
            relative
        );
        activeByInstance.computeIfAbsent(sound.instanceId(), ignored -> new ArrayList<>()).add(instance);
        client.getSoundManager().play(instance, sound.delayTicks());
        return true;
    }

    @Override
    public void stop(long instanceId, int fadeOutTicks) {
        MinecraftClient client = MinecraftClient.getInstance();
        List<FadeableSoundInstance> instances = activeByInstance.remove(instanceId);
        if (client == null || client.getSoundManager() == null || instances == null) {
            return;
        }
        for (FadeableSoundInstance instance : instances) {
            if (fadeOutTicks <= 0) {
                client.getSoundManager().stop(instance);
            } else {
                // 交给 Minecraft 自身的 TickableSoundInstance 循环逐 tick 渐弱；
                // 淡出结束（isDone()==true）后引擎会自己摘除 channel，无需我们再调用
                // soundManager.stop()（那样会打断刚起步的淡出，变回硬切）。
                instance.beginFadeOut(fadeOutTicks);
            }
        }
    }

    private static SoundCategory toMinecraftCategory(AudioCategory category) {
        return switch (category) {
            case MASTER -> SoundCategory.MASTER;
            case PLAYERS -> SoundCategory.PLAYERS;
            case HOSTILE -> SoundCategory.HOSTILE;
            case AMBIENT -> SoundCategory.AMBIENT;
            case VOICE -> SoundCategory.VOICE;
            case BLOCKS -> SoundCategory.BLOCKS;
        };
    }
}
