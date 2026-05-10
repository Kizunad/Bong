package com.bong.client.audio;

public enum AudioCategory {
    MASTER,
    PLAYERS,
    HOSTILE,
    AMBIENT,
    VOICE,
    BLOCKS;

    public static AudioCategory fromWire(String wire) {
        try {
            return AudioCategory.valueOf(wire);
        } catch (RuntimeException ignored) {
            return null;
        }
    }
}
