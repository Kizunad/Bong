package com.bong.client.audio;

public enum AudioBus {
    COMBAT,
    ENVIRONMENT,
    UI;

    public static AudioBus fromWire(String wire) {
        if (wire == null || wire.isBlank()) {
            return null;
        }
        try {
            return AudioBus.valueOf(wire);
        } catch (RuntimeException ignored) {
            return null;
        }
    }

    public static AudioBus fromCategory(AudioCategory category) {
        return switch (category) {
            case HOSTILE, PLAYERS -> COMBAT;
            case AMBIENT, BLOCKS -> ENVIRONMENT;
            case MASTER, VOICE -> UI;
        };
    }
}
