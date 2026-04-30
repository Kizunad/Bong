package com.bong.client.visual.realm_vision;

public record EdgeIndicatorCmd(
    int x,
    int y,
    SenseKind kind,
    double intensity,
    boolean onEdge,
    DirectionBucket bucket
) {
    public EdgeIndicatorCmd {
        kind = kind == null ? SenseKind.LIVING_QI : kind;
        intensity = clamp01(intensity);
        bucket = bucket == null ? DirectionBucket.TOP : bucket;
    }

    private static double clamp01(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        return Math.max(0.0, Math.min(1.0, value));
    }
}
