package com.bong.client.network;

public interface AudioPlaybackBridge {
    boolean play(AudioEventPayload.PlaySoundRecipe payload);

    boolean stop(AudioEventPayload.StopSoundRecipe payload);
}
