package com.bong.client.network;

import com.bong.client.audio.AudioPosition;
import com.bong.client.audio.AudioRecipe;
import com.bong.client.audio.MusicStateMachine;

import java.util.Optional;

public record AmbientZonePayload(
    String zoneName,
    String ambientRecipeId,
    MusicStateMachine.State musicState,
    boolean night,
    String season,
    Optional<String> tsyDepth,
    int fadeTicks,
    Optional<AudioPosition> pos,
    float volumeMul,
    float pitchShift,
    AudioRecipe recipe
) {
    public MusicStateMachine.AmbientZoneUpdate toUpdate() {
        return new MusicStateMachine.AmbientZoneUpdate(
            zoneName,
            ambientRecipeId,
            musicState,
            night,
            season,
            tsyDepth,
            fadeTicks,
            pos,
            volumeMul,
            pitchShift,
            recipe
        );
    }

    public String debugDescriptor() {
        return "ambient_zone " + zoneName + " -> " + musicState + "/" + ambientRecipeId;
    }
}
