package com.bong.client.inventory;

import java.util.Locale;

public final class RarityVisuals {
    private RarityVisuals() {}

    public static String normalize(String rarity) {
        return rarity == null ? "common" : rarity.trim().toLowerCase(Locale.ROOT);
    }

    public static int colorRgb(String rarity) {
        return switch (normalize(rarity)) {
            case "uncommon" -> 0x22CC22;
            case "rare" -> 0x2288FF;
            case "epic" -> 0xAA44FF;
            case "legendary" -> 0xFFAA00;
            case "ancient" -> 0xFF4444;
            default -> 0x808080;
        };
    }

    public static String label(String rarity) {
        return switch (normalize(rarity)) {
            case "uncommon" -> "精良";
            case "rare" -> "稀有";
            case "epic" -> "史诗";
            case "legendary" -> "传说";
            case "ancient" -> "上古";
            default -> "普通";
        };
    }

    public static boolean isAncient(String rarity) {
        return "ancient".equals(normalize(rarity));
    }
}
