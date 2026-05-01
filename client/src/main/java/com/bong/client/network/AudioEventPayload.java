package com.bong.client.network;

import com.bong.client.audio.AudioPosition;
import com.bong.client.audio.AudioRecipe;

import java.util.Optional;

public sealed interface AudioEventPayload permits AudioEventPayload.PlaySoundRecipe, AudioEventPayload.StopSoundRecipe {
    String debugDescriptor();

    record PlaySoundRecipe(
        String recipeId,
        long instanceId,
        Optional<AudioPosition> pos,
        Optional<String> flag,
        float volumeMul,
        float pitchShift,
        AudioRecipe recipe
    ) implements AudioEventPayload {
        public PlaySoundRecipe {
            pos = pos == null ? Optional.empty() : pos;
            flag = flag == null ? Optional.empty() : flag;
        }

        @Override
        public String debugDescriptor() {
            return "play_sound_recipe " + recipeId + "#" + instanceId;
        }
    }

    record StopSoundRecipe(long instanceId, int fadeOutTicks) implements AudioEventPayload {
        @Override
        public String debugDescriptor() {
            return "stop_sound_recipe #" + instanceId;
        }
    }
}
