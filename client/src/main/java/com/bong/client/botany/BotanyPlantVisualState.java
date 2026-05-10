package com.bong.client.botany;

public record BotanyPlantVisualState(float scale, int alpha, int tintRgb, float swayRadians) {
    private static final int MIN_ALPHA = 0;
    private static final int MAX_ALPHA = 255;

    public BotanyPlantVisualState {
        scale = Math.max(0.05f, scale);
        alpha = Math.max(MIN_ALPHA, Math.min(MAX_ALPHA, alpha));
        tintRgb = tintRgb & 0xFFFFFF;
    }

    public static BotanyPlantVisualState forStage(
        PlantGrowthStage stage,
        int baseTintRgb,
        int entityAge,
        float tickDelta
    ) {
        PlantGrowthStage safeStage = stage == null ? PlantGrowthStage.MATURE : stage;
        int tint = baseTintRgb & 0xFFFFFF;
        return switch (safeStage) {
            case SEEDLING -> new BotanyPlantVisualState(0.30f, 128, tint, 0.0f);
            case GROWING -> new BotanyPlantVisualState(
                0.70f,
                210,
                tint,
                (float) Math.sin((entityAge + tickDelta) * 0.12f) * 0.045f
            );
            case WILTED -> new BotanyPlantVisualState(0.92f, 220, desaturate(tint, 0.30f), 0.0f);
            case MATURE -> new BotanyPlantVisualState(1.00f, 255, tint, 0.0f);
        };
    }

    static int desaturate(int rgb, float saturation) {
        float s = Math.max(0.0f, Math.min(1.0f, saturation));
        int r = (rgb >> 16) & 0xFF;
        int g = (rgb >> 8) & 0xFF;
        int b = rgb & 0xFF;
        int grey = Math.round(r * 0.299f + g * 0.587f + b * 0.114f);
        int rr = Math.round(grey + (r - grey) * s);
        int gg = Math.round(grey + (g - grey) * s);
        int bb = Math.round(grey + (b - grey) * s);
        return (rr << 16) | (gg << 8) | bb;
    }
}
