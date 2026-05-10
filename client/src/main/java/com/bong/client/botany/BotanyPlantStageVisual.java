package com.bong.client.botany;

import java.util.Arrays;

public record BotanyPlantStageVisual(
    String key,
    String plantId,
    PlantGrowthStage stage,
    double[] origin,
    int tintRgb,
    double strength,
    long expiresAtTick,
    long updatedAtTick
) {
    public BotanyPlantStageVisual {
        key = normalize(key);
        plantId = normalize(plantId);
        stage = stage == null ? PlantGrowthStage.MATURE : stage;
        origin = normalizeOrigin(origin);
        tintRgb &= 0xFFFFFF;
        strength = Math.max(0.0, Math.min(1.0, strength));
        expiresAtTick = Math.max(0L, expiresAtTick);
        updatedAtTick = Math.max(0L, updatedAtTick);
    }

    public boolean expired(long worldTime) {
        return expiresAtTick <= worldTime;
    }

    public double x() {
        return origin[0];
    }

    public double y() {
        return origin[1];
    }

    public double z() {
        return origin[2];
    }

    private static String normalize(String value) {
        return value == null ? "" : value.trim();
    }

    private static double[] normalizeOrigin(double[] value) {
        if (value == null || value.length != 3) {
            return new double[] { 0.0, 0.0, 0.0 };
        }
        return Arrays.copyOf(value, 3);
    }
}
