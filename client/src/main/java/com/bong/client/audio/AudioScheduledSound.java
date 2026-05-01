package com.bong.client.audio;

import net.minecraft.util.Identifier;

import java.util.Optional;

public record AudioScheduledSound(
    long instanceId,
    Identifier sound,
    AudioCategory category,
    AudioAttenuation attenuation,
    Optional<AudioPosition> pos,
    float volume,
    float pitch,
    int delayTicks
) {
}
