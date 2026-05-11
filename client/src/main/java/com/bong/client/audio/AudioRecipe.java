package com.bong.client.audio;

import java.util.List;
import java.util.Optional;

public record AudioRecipe(
    String id,
    List<AudioLayer> layers,
    Optional<AudioLoopConfig> loop,
    int priority,
    AudioAttenuation attenuation,
    AudioCategory category,
    AudioBus bus
) {
    public AudioRecipe {
        layers = List.copyOf(layers);
        loop = loop == null ? Optional.empty() : loop;
        bus = bus == null ? AudioBus.fromCategory(category) : bus;
    }
}
