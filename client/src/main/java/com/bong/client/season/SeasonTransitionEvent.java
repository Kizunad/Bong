package com.bong.client.season;

import com.bong.client.state.SeasonState;

public record SeasonTransitionEvent(
    SeasonState.Phase from,
    SeasonState.Phase to,
    double progress,
    long worldTick
) {
    public SeasonTransitionEvent {
        to = to == null ? SeasonState.Phase.SUMMER : to;
        progress = clamp01(progress);
        worldTick = Math.max(0L, worldTick);
    }

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }
}
