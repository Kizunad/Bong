package com.bong.client.season;

import com.bong.client.botany.BotanyPlantVisualState;
import com.bong.client.state.SeasonState;

import java.util.Locale;

public final class SeasonPlantVisuals {
    private SeasonPlantVisuals() {
    }

    public static BotanyPlantVisualState apply(
        String plantId,
        BotanyPlantVisualState base,
        SeasonState season,
        long worldTick
    ) {
        BotanyPlantVisualState safeBase = base == null ? new BotanyPlantVisualState(1.0f, 255, 0x88AA55, 0.0f) : base;
        if (season == null) {
            return safeBase;
        }
        String id = plantId == null ? "" : plantId.toLowerCase(Locale.ROOT);
        return switch (season.phase()) {
            case SUMMER -> summerVisual(id, safeBase);
            case WINTER -> winterVisual(id, safeBase, season);
            case SUMMER_TO_WINTER, WINTER_TO_SUMMER -> tideTurnVisual(safeBase, worldTick);
        };
    }

    private static BotanyPlantVisualState summerVisual(String plantId, BotanyPlantVisualState base) {
        if (isHeatTolerant(plantId)) {
            return new BotanyPlantVisualState(
                base.scale() * 1.04f,
                base.alpha(),
                blend(base.tintRgb(), 0xFFD36A, 0.25f),
                base.swayRadians() * 1.20f
            );
        }
        return new BotanyPlantVisualState(
            base.scale(),
            base.alpha(),
            blend(desaturate(base.tintRgb(), 0.45f), 0x8A5A2B, 0.35f),
            base.swayRadians() * 0.65f
        );
    }

    private static BotanyPlantVisualState winterVisual(String plantId, BotanyPlantVisualState base, SeasonState season) {
        if (isFrostSpecies(plantId)) {
            int fade = Math.max(32, Math.round(base.alpha() * Math.min(1.0f, (float) progress(season) * 5.0f)));
            return new BotanyPlantVisualState(base.scale(), fade, blend(base.tintRgb(), 0xC0E0FF, 0.45f), 0.0f);
        }
        if (isColdTolerant(plantId)) {
            return new BotanyPlantVisualState(base.scale(), base.alpha(), blend(base.tintRgb(), 0x80C8FF, 0.30f), base.swayRadians() * 0.35f);
        }
        return new BotanyPlantVisualState(base.scale(), base.alpha(), blend(desaturate(base.tintRgb(), 0.25f), 0xF0F8FF, 0.55f), 0.0f);
    }

    private static BotanyPlantVisualState tideTurnVisual(BotanyPlantVisualState base, long worldTick) {
        double pulse = Math.sin(worldTick / 60.0 * Math.PI * 2.0);
        float scale = (float) (base.scale() * (1.0 + pulse * 0.08));
        return new BotanyPlantVisualState(scale, base.alpha(), blend(base.tintRgb(), 0x9966CC, 0.22f), base.swayRadians() + (float) pulse * 0.035f);
    }

    public static boolean isFrostSpecies(String plantId) {
        String id = plantId == null ? "" : plantId.toLowerCase(Locale.ROOT);
        return id.contains("xue_po") || id.contains("shuang") || id.contains("frost");
    }

    private static boolean isHeatTolerant(String plantId) {
        return plantId.contains("chi_")
            || plantId.contains("yan")
            || plantId.contains("lie_")
            || plantId.contains("yang_");
    }

    private static boolean isColdTolerant(String plantId) {
        return isFrostSpecies(plantId)
            || plantId.contains("ning_")
            || plantId.contains("xue_")
            || plantId.contains("bei_wen");
    }

    private static double progress(SeasonState season) {
        return Math.max(0.0, Math.min(1.0, (double) season.tickIntoPhase() / (double) season.phaseTotalTicks()));
    }

    private static int desaturate(int rgb, float saturation) {
        int r = (rgb >> 16) & 0xFF;
        int g = (rgb >> 8) & 0xFF;
        int b = rgb & 0xFF;
        int grey = Math.round(r * 0.299f + g * 0.587f + b * 0.114f);
        int rr = Math.round(grey + (r - grey) * saturation);
        int gg = Math.round(grey + (g - grey) * saturation);
        int bb = Math.round(grey + (b - grey) * saturation);
        return (rr << 16) | (gg << 8) | bb;
    }

    private static int blend(int from, int to, float amount) {
        float a = Math.max(0.0f, Math.min(1.0f, amount));
        int r = Math.round(((from >> 16) & 0xFF) * (1.0f - a) + ((to >> 16) & 0xFF) * a);
        int g = Math.round(((from >> 8) & 0xFF) * (1.0f - a) + ((to >> 8) & 0xFF) * a);
        int b = Math.round((from & 0xFF) * (1.0f - a) + (to & 0xFF) * a);
        return (r << 16) | (g << 8) | b;
    }
}
