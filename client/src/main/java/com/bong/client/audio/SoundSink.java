package com.bong.client.audio;

public interface SoundSink {
    boolean play(AudioScheduledSound sound);

    default void stop(long instanceId, int fadeOutTicks) {
    }
}
