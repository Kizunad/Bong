package com.bong.client.visual.realm_vision;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.EnumMap;
import java.util.List;
import java.util.Map;

public final class PerceptionEdgeProjector {
    public static final int MAX_PER_DIRECTION = 3;
    private static final int EDGE_MARGIN = 6;

    private PerceptionEdgeProjector() {
    }

    public static EdgeIndicatorCmd project(
        double worldX, double worldY, double worldZ,
        double camX, double camY, double camZ,
        float yawDeg, float pitchDeg,
        double fovDegVertical,
        int scaledWidth, int scaledHeight,
        SenseKind kind,
        double intensity
    ) {
        if (scaledWidth <= 0 || scaledHeight <= 0) {
            return new EdgeIndicatorCmd(0, 0, kind, intensity, false, DirectionBucket.TOP);
        }

        double yaw = Math.toRadians(yawDeg);
        double pitch = Math.toRadians(pitchDeg);
        double cp = Math.cos(pitch);
        double fx = -Math.sin(yaw) * cp;
        double fy = -Math.sin(pitch);
        double fz = Math.cos(yaw) * cp;
        double rx = Math.cos(yaw);
        double rz = Math.sin(yaw);
        double ux = fy * rz;
        double uy = fz * rx - fx * rz;
        double uz = -fy * rx;

        double dx = worldX - camX;
        double dy = worldY - camY;
        double dz = worldZ - camZ;
        double vf = dx * fx + dy * fy + dz * fz;
        double vr = dx * rx + dz * rz;
        double vu = dx * ux + dy * uy + dz * uz;
        boolean behind = vf <= 0.05;
        if (behind) {
            vf = Math.max(0.05, Math.abs(vf));
            vr = -vr;
            vu = -vu;
            if (Math.abs(vr) < 0.0001 && Math.abs(vu) < 0.0001) {
                vu = -vf;
            }
        }

        double tanHalfV = Math.tan(Math.toRadians(fovDegVertical) * 0.5);
        double tanHalfH = tanHalfV * ((double) scaledWidth / (double) scaledHeight);
        double ndcX = (vr / vf) / tanHalfH;
        double ndcY = (vu / vf) / tanHalfV;
        boolean inside = !behind && Math.abs(ndcX) <= 1.0 && Math.abs(ndcY) <= 1.0;
        if (inside) {
            int x = (int) Math.round((ndcX * 0.5 + 0.5) * scaledWidth);
            int y = (int) Math.round((0.5 - ndcY * 0.5) * scaledHeight);
            return new EdgeIndicatorCmd(x, y, kind, intensity, false, bucketFor(ndcX, ndcY));
        }

        double scale = 1.0 / Math.max(Math.abs(ndcX), Math.abs(ndcY));
        double edgeX = ndcX * scale;
        double edgeY = ndcY * scale;
        int sx = clamp((int) Math.round((edgeX * 0.5 + 0.5) * scaledWidth), EDGE_MARGIN, scaledWidth - EDGE_MARGIN);
        int sy = clamp((int) Math.round((0.5 - edgeY * 0.5) * scaledHeight), EDGE_MARGIN, scaledHeight - EDGE_MARGIN);
        return new EdgeIndicatorCmd(sx, sy, kind, intensity, true, bucketFor(edgeX, edgeY));
    }

    public static List<EdgeIndicatorCmd> capPerDirection(List<EdgeIndicatorCmd> commands) {
        if (commands == null || commands.isEmpty()) {
            return List.of();
        }
        Map<DirectionBucket, Integer> counts = new EnumMap<>(DirectionBucket.class);
        List<EdgeIndicatorCmd> sorted = new ArrayList<>(commands.stream().filter(EdgeIndicatorCmd::onEdge).toList());
        sorted.sort(Comparator.comparingDouble(EdgeIndicatorCmd::intensity).reversed());
        List<EdgeIndicatorCmd> out = new ArrayList<>();
        for (EdgeIndicatorCmd command : sorted) {
            int count = counts.getOrDefault(command.bucket(), 0);
            if (count >= MAX_PER_DIRECTION) {
                continue;
            }
            counts.put(command.bucket(), count + 1);
            out.add(command);
        }
        return out;
    }

    private static DirectionBucket bucketFor(double ndcX, double ndcY) {
        if (Math.abs(ndcX) > Math.abs(ndcY)) {
            return ndcX < 0.0 ? DirectionBucket.LEFT : DirectionBucket.RIGHT;
        }
        return ndcY < 0.0 ? DirectionBucket.BOTTOM : DirectionBucket.TOP;
    }

    private static int clamp(int value, int min, int max) {
        if (max < min) {
            return value;
        }
        return Math.max(min, Math.min(max, value));
    }
}
