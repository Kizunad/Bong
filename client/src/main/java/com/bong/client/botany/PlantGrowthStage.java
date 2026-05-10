package com.bong.client.botany;

import java.util.Locale;

public enum PlantGrowthStage {
    SEEDLING,
    GROWING,
    MATURE,
    WILTED;

    public static PlantGrowthStage fromWireName(String raw) {
        if (raw == null || raw.isBlank()) {
            return MATURE;
        }
        return switch (raw.trim().toLowerCase(Locale.ROOT)) {
            case "seedling" -> SEEDLING;
            case "growing" -> GROWING;
            case "wilted" -> WILTED;
            default -> MATURE;
        };
    }

    public String wireName() {
        return name().toLowerCase(Locale.ROOT);
    }
}
