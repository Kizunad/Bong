package com.bong.client.audio;

public enum AudioAttenuation {
    PLAYER_LOCAL,
    WORLD_3D,
    GLOBAL_HINT,
    ZONE_BROADCAST;

    public static AudioAttenuation fromWire(String wire) {
        return switch (wire) {
            case "player_local" -> PLAYER_LOCAL;
            case "world_3d" -> WORLD_3D;
            case "global_hint" -> GLOBAL_HINT;
            case "zone_broadcast" -> ZONE_BROADCAST;
            default -> null;
        };
    }
}
