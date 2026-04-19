package com.bong.client.hud;

/**
 * plan-botany-v1 §1.3 投影锚定：世界坐标 → scaled HUD 像素。
 * <p>MC 1.20.1 惯例：yaw=0 面朝南（+Z）；pitch>0 面朝下。相机 forward 向量：
 * <pre>f = (-sin(yaw)*cos(pitch), -sin(pitch), cos(yaw)*cos(pitch))</pre>
 * 右向量 r = (cos(yaw), 0, sin(yaw))，上向量 u = f × r。
 */
public final class BotanyProjection {
    private BotanyProjection() {
    }

    public record Anchor(int x, int y, boolean visible) {
        public static Anchor invisible() {
            return new Anchor(0, 0, false);
        }
    }

    public static Anchor project(
        double worldX, double worldY, double worldZ,
        double camX, double camY, double camZ,
        float yawDeg, float pitchDeg,
        double fovDegVertical,
        int scaledWidth, int scaledHeight
    ) {
        if (scaledWidth <= 0 || scaledHeight <= 0) {
            return Anchor.invisible();
        }

        double yaw = Math.toRadians(yawDeg);
        double pitch = Math.toRadians(pitchDeg);
        double cp = Math.cos(pitch);

        double fx = -Math.sin(yaw) * cp;
        double fy = -Math.sin(pitch);
        double fz = Math.cos(yaw) * cp;

        double rx = Math.cos(yaw);
        double rz = Math.sin(yaw);

        // up = forward × right（ry = 0）
        double ux = fy * rz;
        double uy = fz * rx - fx * rz;
        double uz = -fy * rx;

        double dx = worldX - camX;
        double dy = worldY - camY;
        double dz = worldZ - camZ;

        double vf = dx * fx + dy * fy + dz * fz;
        if (vf <= 0.05) {
            // 目标在相机之后或几乎贴脸
            return Anchor.invisible();
        }

        double vr = dx * rx + dz * rz;
        double vu = dx * ux + dy * uy + dz * uz;

        double tanHalfV = Math.tan(Math.toRadians(fovDegVertical) * 0.5);
        double aspect = (double) scaledWidth / (double) scaledHeight;
        double tanHalfH = tanHalfV * aspect;

        double ndcX = (vr / vf) / tanHalfH;
        double ndcY = (vu / vf) / tanHalfV;

        int sx = (int) Math.round((ndcX * 0.5 + 0.5) * scaledWidth);
        int sy = (int) Math.round((0.5 - ndcY * 0.5) * scaledHeight);
        boolean visible = Math.abs(ndcX) <= 1.0 && Math.abs(ndcY) <= 1.0;
        return new Anchor(sx, sy, visible);
    }
}
