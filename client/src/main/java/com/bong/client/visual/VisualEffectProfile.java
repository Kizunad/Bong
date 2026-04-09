package com.bong.client.visual;

import com.bong.client.state.VisualEffectState;

enum VisualEffectProfile {
    SYSTEM_WARNING(
        VisualEffectState.EffectType.SCREEN_SHAKE,
        0xF07C3E,
        0.85,
        2_400L,
        1_200L,
        255,
        "≋ 天道警示 ≋"
    ),
    PERCEPTION(
        VisualEffectState.EffectType.FOG_TINT,
        0x5F7693,
        0.55,
        4_500L,
        1_500L,
        96,
        null
    ),
    ERA_DECREE(
        VisualEffectState.EffectType.TITLE_FLASH,
        0xF2CC6B,
        0.75,
        3_200L,
        2_200L,
        255,
        "✦ 时代法旨 ✦"
    );

    private final VisualEffectState.EffectType effectType;
    private final int baseColor;
    private final double maxIntensity;
    private final long maxDurationMillis;
    private final long retriggerWindowMillis;
    private final int maxAlpha;
    private final String overlayLabel;

    VisualEffectProfile(
        VisualEffectState.EffectType effectType,
        int baseColor,
        double maxIntensity,
        long maxDurationMillis,
        long retriggerWindowMillis,
        int maxAlpha,
        String overlayLabel
    ) {
        this.effectType = effectType;
        this.baseColor = baseColor;
        this.maxIntensity = maxIntensity;
        this.maxDurationMillis = maxDurationMillis;
        this.retriggerWindowMillis = retriggerWindowMillis;
        this.maxAlpha = maxAlpha;
        this.overlayLabel = overlayLabel;
    }

    static VisualEffectProfile from(VisualEffectState visualEffectState) {
        if (visualEffectState == null || visualEffectState.isEmpty()) {
            return null;
        }

        return switch (visualEffectState.effectType()) {
            case SCREEN_SHAKE -> SYSTEM_WARNING;
            case FOG_TINT -> PERCEPTION;
            case TITLE_FLASH -> ERA_DECREE;
            case NONE -> null;
        };
    }

    VisualEffectState.EffectType effectType() {
        return effectType;
    }

    int baseColor() {
        return baseColor;
    }

    double maxIntensity() {
        return maxIntensity;
    }

    long maxDurationMillis() {
        return maxDurationMillis;
    }

    long retriggerWindowMillis() {
        return retriggerWindowMillis;
    }

    int maxAlpha() {
        return maxAlpha;
    }

    String overlayLabel() {
        return overlayLabel;
    }
}
