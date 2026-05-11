package com.bong.client.audio;

import java.util.EnumMap;
import java.util.Map;

public final class AudioBusMixer {
    private final Map<AudioBus, Float> volumes = new EnumMap<>(AudioBus.class);
    private MusicStateMachine.State musicState = MusicStateMachine.State.AMBIENT;
    private boolean immersiveMode;
    private int uiRestoreTicks;

    public AudioBusMixer() {
        for (AudioBus bus : AudioBus.values()) {
            volumes.put(bus, 1.0f);
        }
    }

    public void setVolume(AudioBus bus, float volume) {
        volumes.put(bus, clamp01(volume));
    }

    public void setImmersiveMode(boolean immersiveMode) {
        this.immersiveMode = immersiveMode;
    }

    public void restoreUiForTicks(int ticks) {
        uiRestoreTicks = Math.max(uiRestoreTicks, Math.max(0, ticks));
    }

    public void setMusicState(MusicStateMachine.State musicState) {
        this.musicState = musicState == null ? MusicStateMachine.State.AMBIENT : musicState;
    }

    public void tick() {
        if (uiRestoreTicks > 0) {
            uiRestoreTicks--;
        }
    }

    public float effectiveVolume(AudioBus bus) {
        AudioBus resolved = bus == null ? AudioBus.ENVIRONMENT : bus;
        float volume = volumes.getOrDefault(resolved, 1.0f);
        if (resolved == AudioBus.UI && immersiveMode && uiRestoreTicks <= 0) {
            return 0.0f;
        }
        if (resolved == AudioBus.ENVIRONMENT) {
            if (musicState == MusicStateMachine.State.TRIBULATION) {
                return volume * 0.3f;
            }
            if (musicState == MusicStateMachine.State.COMBAT) {
                return volume * 0.6f;
            }
        }
        return volume;
    }

    private static float clamp01(float value) {
        if (!Float.isFinite(value)) {
            return 1.0f;
        }
        return Math.max(0.0f, Math.min(1.0f, value));
    }
}
