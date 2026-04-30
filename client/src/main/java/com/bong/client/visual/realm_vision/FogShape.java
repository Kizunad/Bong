package com.bong.client.visual.realm_vision;

public enum FogShape {
    CYLINDER,
    SPHERE;

    public static FogShape fromWire(String wireName) {
        if (wireName == null) {
            return CYLINDER;
        }
        return switch (wireName.trim()) {
            case "Sphere", "sphere" -> SPHERE;
            default -> CYLINDER;
        };
    }
}
