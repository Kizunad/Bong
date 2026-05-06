package com.bong.client.visual.season;

import com.bong.client.state.SeasonState;

public final class SeasonVisuals {
    private SeasonVisuals() {
    }

    public static int qiBarColor(int baseColor, SeasonState state, long nowMillis) {
        if (state == null) {
            return baseColor;
        }
        return switch (state.phase()) {
            case SUMMER -> adjustSaturation(baseColor, 0.90);
            case WINTER -> adjustSaturation(baseColor, 1.10);
            case SUMMER_TO_WINTER, WINTER_TO_SUMMER -> {
                double pulse = ((nowMillis / 250L) & 1L) == 0L ? 0.78 : 1.22;
                yield adjustSaturation(baseColor, pulse);
            }
        };
    }

    public static int skyTintArgb(SeasonState state, long nowMillis) {
        if (state == null) {
            return 0;
        }
        return switch (state.phase()) {
            case SUMMER -> 0x10FFB060;
            case WINTER -> 0x104080FF;
            case SUMMER_TO_WINTER, WINTER_TO_SUMMER ->
                ((nowMillis / 350L) & 1L) == 0L ? 0x12FFB060 : 0x124080FF;
        };
    }

    public static ParticleCue particleCue(SeasonState state, long worldTick) {
        if (state == null || state.phase().tideTurn()) {
            return ParticleCue.none();
        }
        if (state.phase() == SeasonState.Phase.SUMMER) {
            return worldTick % 12L == 0L
                ? new ParticleCue(ParticleKind.HEAT_SHIMMER, 1, 0.015)
                : ParticleCue.none();
        }
        return worldTick % 10L == 0L
            ? new ParticleCue(ParticleKind.SNOW_GRAIN, 1, -0.015)
            : ParticleCue.none();
    }

    private static int adjustSaturation(int argb, double factor) {
        int alpha = argb & 0xFF000000;
        int r = (argb >>> 16) & 0xFF;
        int g = (argb >>> 8) & 0xFF;
        int b = argb & 0xFF;
        int gray = (int) Math.round(r * 0.299 + g * 0.587 + b * 0.114);
        return alpha
            | (channel(gray + (r - gray) * factor) << 16)
            | (channel(gray + (g - gray) * factor) << 8)
            | channel(gray + (b - gray) * factor);
    }

    private static int channel(double value) {
        return Math.max(0, Math.min(255, (int) Math.round(value)));
    }

    public enum ParticleKind {
        NONE,
        HEAT_SHIMMER,
        SNOW_GRAIN
    }

    public record ParticleCue(ParticleKind kind, int count, double yVelocity) {
        public ParticleCue {
            kind = kind == null ? ParticleKind.NONE : kind;
            count = Math.max(0, count);
        }

        public static ParticleCue none() {
            return new ParticleCue(ParticleKind.NONE, 0, 0.0);
        }
    }
}
