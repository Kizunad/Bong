package com.bong.client.spirittreasure;

public record SpiritTreasurePassive(String kind, double value, String description) {
    public SpiritTreasurePassive {
        kind = sanitize(kind);
        description = sanitize(description);
    }

    private static String sanitize(String value) {
        return value == null ? "" : value.trim();
    }
}
