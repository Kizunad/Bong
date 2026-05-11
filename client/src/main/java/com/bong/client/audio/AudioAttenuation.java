package com.bong.client.audio;

public enum AudioAttenuation {
    PLAYER_LOCAL,
    WORLD_3D,
    GLOBAL_HINT,
    ZONE_BROADCAST,
    SELF,
    MELEE,
    AREA,
    WORLD;

    public static AudioAttenuation fromWire(String wire) {
        return switch (wire) {
            case "player_local" -> PLAYER_LOCAL;
            case "world_3d" -> WORLD_3D;
            case "global_hint" -> GLOBAL_HINT;
            case "zone_broadcast" -> ZONE_BROADCAST;
            case "SELF" -> SELF;
            case "MELEE" -> MELEE;
            case "AREA" -> AREA;
            case "WORLD" -> WORLD;
            default -> null;
        };
    }

    public int radiusBlocks() {
        return switch (this) {
            case PLAYER_LOCAL, SELF -> 0;
            case MELEE -> 8;
            case AREA -> 32;
            case WORLD -> 128;
            case WORLD_3D, ZONE_BROADCAST -> 64;
            case GLOBAL_HINT -> Integer.MAX_VALUE;
        };
    }
}
