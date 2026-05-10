package com.bong.client.hud;

import java.util.List;

public record HudRuntimeContext(
    double yawDegrees,
    double playerX,
    double playerY,
    double playerZ,
    boolean altPeekDown,
    List<CompassMarker> compassMarkers
) {
    public HudRuntimeContext {
        yawDegrees = normalizeDegrees(yawDegrees);
        if (!Double.isFinite(playerX)) playerX = 0.0;
        if (!Double.isFinite(playerY)) playerY = 0.0;
        if (!Double.isFinite(playerZ)) playerZ = 0.0;
        compassMarkers = compassMarkers == null ? List.of() : List.copyOf(compassMarkers);
    }

    public static HudRuntimeContext empty() {
        return new HudRuntimeContext(0.0, 0.0, 0.0, 0.0, false, List.of());
    }

    public HudRuntimeContext withCompassMarkers(List<CompassMarker> markers) {
        return new HudRuntimeContext(yawDegrees, playerX, playerY, playerZ, altPeekDown, markers);
    }

    public static double normalizeDegrees(double value) {
        if (!Double.isFinite(value)) {
            return 0.0;
        }
        double normalized = value % 360.0;
        return normalized < 0.0 ? normalized + 360.0 : normalized;
    }

    public record CompassMarker(Kind kind, double worldX, double worldZ, double intensity) {
        public CompassMarker {
            kind = kind == null ? Kind.TSY_PORTAL : kind;
            if (!Double.isFinite(worldX)) worldX = 0.0;
            if (!Double.isFinite(worldZ)) worldZ = 0.0;
            intensity = Math.max(0.0, Math.min(1.0, Double.isFinite(intensity) ? intensity : 0.0));
        }

        public enum Kind {
            SPIRIT_NICHE,
            TSY_PORTAL,
            COLLAPSE_EXIT
        }
    }
}
