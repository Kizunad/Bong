package com.bong.client.season;

import com.bong.client.state.SeasonState;

public final class SeasonBreakthroughOverlay {
    private SeasonBreakthroughOverlay() {
    }

    public static BreakthroughProfile breakthroughProfile(SeasonState state, boolean success, long worldTick) {
        SeasonState.Phase phase = state == null ? SeasonState.Phase.SUMMER : state.phase();
        return switch (phase) {
            case SUMMER -> success
                ? new BreakthroughProfile(0xFFD36A, 1.50, "tribulation_spark", 0x00000000, 0.0)
                : new BreakthroughProfile(0xFF5533, 1.20, "breakthrough_fail", 0x30FF5533, 0.25);
            case WINTER -> success
                ? new BreakthroughProfile(0xC0E0FF, 1.00, "enlightenment_dust", 0x204080FF, 0.0)
                : new BreakthroughProfile(0x70A8FF, 0.90, "breakthrough_fail", 0x304080FF, 0.18);
            case SUMMER_TO_WINTER, WINTER_TO_SUMMER -> {
                int tint = (worldTick / 4L) % 3L == 0L ? 0xFFD36A : (worldTick / 4L) % 3L == 1L ? 0xC0E0FF : 0x9966CC;
                yield new BreakthroughProfile(tint, 1.30, "tribulation_spark", 0x309966CC, 0.55);
            }
        };
    }

    public static MeditationProfile meditationAbsorbProfile(SeasonState state, long worldTick) {
        SeasonState.Phase phase = state == null ? SeasonState.Phase.SUMMER : state.phase();
        return switch (phase) {
            case SUMMER -> new MeditationProfile(1.20, 1.30, false);
            case WINTER -> new MeditationProfile(0.60, 0.50, false);
            case SUMMER_TO_WINTER, WINTER_TO_SUMMER -> {
                double wave = 1.0 + Math.sin(worldTick / 20.0) * 0.35;
                yield new MeditationProfile(wave, wave, true);
            }
        };
    }

    public record BreakthroughProfile(
        int pillarTintRgb,
        double lightningMultiplier,
        String particleSpriteId,
        int screenPulseArgb,
        double backlashIntensity
    ) {
        public BreakthroughProfile {
            pillarTintRgb &= 0xFFFFFF;
            lightningMultiplier = finiteAtLeastZero(lightningMultiplier);
            particleSpriteId = particleSpriteId == null ? "" : particleSpriteId.trim();
            backlashIntensity = clamp01(backlashIntensity);
        }
    }

    public record MeditationProfile(
        double densityMultiplier,
        double velocityMultiplier,
        boolean allowsReverseBounce
    ) {
        public MeditationProfile {
            densityMultiplier = finiteAtLeastZero(densityMultiplier);
            velocityMultiplier = finiteAtLeastZero(velocityMultiplier);
        }
    }

    private static double finiteAtLeastZero(double value) {
        return Double.isFinite(value) ? Math.max(0.0, value) : 0.0;
    }

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }
}
