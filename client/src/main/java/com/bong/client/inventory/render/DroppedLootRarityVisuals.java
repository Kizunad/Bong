package com.bong.client.inventory.render;

import com.bong.client.inventory.RarityBorderRenderer;

import java.util.Locale;

public final class DroppedLootRarityVisuals {
    private DroppedLootRarityVisuals() {}

    public static boolean hasAuraParticles(String rarity) {
        return rarityRank(rarity) >= rarityRank("rare");
    }

    public static int auraParticleCount(String rarity) {
        return switch (normalize(rarity)) {
            case "legendary", "ancient" -> 4;
            case "rare", "epic" -> 2;
            default -> 0;
        };
    }

    public static double beamHeight(String rarity) {
        return switch (normalize(rarity)) {
            case "legendary" -> 1.0;
            case "ancient" -> 1.35;
            default -> 0.0;
        };
    }

    public static boolean shouldHum(String rarity) {
        return "ancient".equals(normalize(rarity));
    }

    public static boolean isAncient(String rarity) {
        return "ancient".equals(normalize(rarity));
    }

    public static float red(String rarity) {
        return ((RarityBorderRenderer.colorRgb(rarity) >> 16) & 0xFF) / 255.0f;
    }

    public static float green(String rarity) {
        return ((RarityBorderRenderer.colorRgb(rarity) >> 8) & 0xFF) / 255.0f;
    }

    public static float blue(String rarity) {
        return (RarityBorderRenderer.colorRgb(rarity) & 0xFF) / 255.0f;
    }

    private static int rarityRank(String rarity) {
        return switch (normalize(rarity)) {
            case "uncommon" -> 1;
            case "rare" -> 2;
            case "epic" -> 3;
            case "legendary" -> 4;
            case "ancient" -> 5;
            default -> 0;
        };
    }

    private static String normalize(String rarity) {
        return rarity == null ? "common" : rarity.trim().toLowerCase(Locale.ROOT);
    }
}
