package com.bong.client.audio;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.sound.PositionedSoundInstance;
import net.minecraft.client.sound.SoundInstance;
import net.minecraft.sound.SoundCategory;
import net.minecraft.util.math.random.Random;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

public final class MinecraftSoundSink implements SoundSink {
    private final Map<Long, List<SoundInstance>> activeByInstance = new ConcurrentHashMap<>();

    @Override
    public boolean play(AudioScheduledSound sound) {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null || client.getSoundManager() == null) {
            return false;
        }

        boolean relative = sound.attenuation() == AudioAttenuation.PLAYER_LOCAL;
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

        PositionedSoundInstance instance = new PositionedSoundInstance(
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
        List<SoundInstance> instances = activeByInstance.remove(instanceId);
        if (client == null || client.getSoundManager() == null || instances == null) {
            return;
        }
        for (SoundInstance instance : instances) {
            client.getSoundManager().stop(instance);
        }
    }

    private static SoundCategory toMinecraftCategory(AudioCategory category) {
        return switch (category) {
            case MASTER -> SoundCategory.MASTER;
            case HOSTILE -> SoundCategory.HOSTILE;
            case AMBIENT -> SoundCategory.AMBIENT;
            case VOICE -> SoundCategory.VOICE;
            case BLOCKS -> SoundCategory.BLOCKS;
        };
    }
}
