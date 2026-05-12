package com.bong.client.cultivation;

public final class DistantBreakthroughRenderer {
    private static final double NEAR_VIEWER_DISTANCE = 64.0;

    private DistantBreakthroughRenderer() {}

    public static Billboard billboardFor(
        BreakthroughCinematicPayload payload,
        double viewerX,
        double viewerY,
        double viewerZ
    ) {
        if (payload == null || !payload.distantBillboard()) {
            return Billboard.hidden();
        }
        double dx = payload.worldX() - viewerX;
        double dy = payload.worldY() - viewerY;
        double dz = payload.worldZ() - viewerZ;
        double distance = Math.sqrt(dx * dx + dy * dy + dz * dz);
        if (!Double.isFinite(distance) || distance <= NEAR_VIEWER_DISTANCE) {
            return Billboard.hidden();
        }
        if (!payload.global() && distance > payload.visibleRadiusBlocks()) {
            return Billboard.hidden();
        }

        double yawRadians = Math.atan2(dz, dx);
        double pitchRadians = Math.atan2(dy, Math.max(1.0, Math.sqrt(dx * dx + dz * dz)));
        double normalized = payload.global()
            ? Math.min(1.0, distance / Math.max(NEAR_VIEWER_DISTANCE, payload.visibleRadiusBlocks()))
            : distance / payload.visibleRadiusBlocks();
        double alpha = clamp(1.0 - normalized * 0.55, 0.25, 0.9);
        double scale = clamp(1.35 - normalized * 0.5, 0.35, 1.4);
        return new Billboard(true, yawRadians, pitchRadians, scale, alpha, tintFor(payload));
    }

    private static int tintFor(BreakthroughCinematicPayload payload) {
        return switch (payload.realmTo()) {
            case "Condense" -> 0xCC88CCDD;
            case "Solidify" -> 0xCCFFD700;
            case "Spirit" -> 0xCCFFF3B0;
            case "Void" -> 0xCCB445FF;
            default -> 0xCC66FFCC;
        };
    }

    private static double clamp(double value, double min, double max) {
        if (!Double.isFinite(value)) return min;
        return Math.max(min, Math.min(max, value));
    }

    public record Billboard(
        boolean visible,
        double yawRadians,
        double pitchRadians,
        double scale,
        double alpha,
        int tintArgb
    ) {
        public static Billboard hidden() {
            return new Billboard(false, 0.0, 0.0, 0.0, 0.0, 0);
        }
    }
}
