package com.bong.client.visual.particle;

/**
 * 纯几何工具：为三个 Bong 粒子基类（Line / Ribbon / GroundDecal）计算四边形顶点。
 *
 * <p>抽出来放在这里纯粹为了<strong>可单元测试</strong>——MC 的 {@code VertexConsumer} / {@code Camera}
 * 是客户端运行时对象，单测环境构造不出来。把向量数学留在这里，渲染类只负责把结果灌入
 * {@code VertexConsumer}。
 *
 * <p>所有返回的 {@code float[12]} 都是"4 个顶点 × 3 个分量"连续数组（[x0,y0,z0, x1,y1,z1, ...]），
 * 顶点顺序为逆时针（从正面看），配合 MC 的 CCW 前面剔除。
 *
 * <p>坐标系约定：右手系，+Y 朝上，camera-relative。调用方已经减去 camera 位置。
 *
 * <p>plan-particle-system-v1 §1.1 / §1.2 / §1.3。
 */
public final class BongParticleGeometry {
    private BongParticleGeometry() {
    }

    private static final double EPSILON = 1e-6;
    private static final double GROUND_DECAL_MAX_SNAP_UP = 0.25;
    private static final double GROUND_DECAL_MAX_SNAP_DOWN = 2.0;
    /** 速度数量级极小（近似静止）时的后备方向（世界 +X）。防止 normalize 出 NaN。 */
    private static final double[] FALLBACK_FORWARD = { 1.0, 0.0, 0.0 };
    /** 与 forward 平行时的次级参考（世界 +Y）。GroundDecal 用 +Z。 */
    private static final double[] UP_HINT = { 0.0, 1.0, 0.0 };

    /**
     * 沿速度方向拉长的 quad（plan §1.1 {@code BongLineParticle}）。
     *
     * <p>quad 的长轴沿 {@code velocity}（归一化后），宽轴垂直于长轴<em>且</em>尽量接近水平——
     * 这是剑气/暗器轨迹的直觉方向。速度接近 0 时回退到 {@link #FALLBACK_FORWARD} 防崩。
     *
     * @param center   quad 中心点（相对 camera）
     * @param velocity 世界速度矢量
     * @param lengthFactor length = |velocity| * lengthFactor（plan "长度 = 速度 × factor"）
     * @param minLength    保底最短长度，防止 velocity 极小时 quad 退化成点
     * @param halfWidth    quad 半宽
     * @return 4 顶点 × 3 分量 的顺序顶点数组
     */
    public static float[] buildLineQuad(
        double[] center,
        double[] velocity,
        double lengthFactor,
        double minLength,
        double halfWidth
    ) {
        double vLen = length3(velocity);
        double[] forward;
        if (vLen < EPSILON) {
            forward = FALLBACK_FORWARD.clone();
        } else {
            forward = new double[] {
                velocity[0] / vLen,
                velocity[1] / vLen,
                velocity[2] / vLen,
            };
        }
        double length = Math.max(minLength, vLen * lengthFactor);
        double halfLen = length * 0.5;

        // width 轴：取 forward × up_hint；forward 与 up 平行时，换 +Z 作 hint
        double[] width = cross3(forward, UP_HINT);
        double wLen = length3(width);
        if (wLen < EPSILON) {
            width = cross3(forward, new double[] { 0.0, 0.0, 1.0 });
            wLen = length3(width);
            if (wLen < EPSILON) {
                // 仍然退化（不应该发生：forward 同时平行 +Y 和 +Z 不可能）
                width = new double[] { 0.0, 0.0, 1.0 };
                wLen = 1.0;
            }
        }
        double sx = width[0] / wLen * halfWidth;
        double sy = width[1] / wLen * halfWidth;
        double sz = width[2] / wLen * halfWidth;

        double fx = forward[0] * halfLen;
        double fy = forward[1] * halfLen;
        double fz = forward[2] * halfLen;

        // 逆时针：尾端下 → 尾端上 → 头端上 → 头端下（俯视 width 方向）
        float[] out = new float[12];
        out[0]  = (float)(center[0] - fx - sx);
        out[1]  = (float)(center[1] - fy - sy);
        out[2]  = (float)(center[2] - fz - sz);
        out[3]  = (float)(center[0] - fx + sx);
        out[4]  = (float)(center[1] - fy + sy);
        out[5]  = (float)(center[2] - fz + sz);
        out[6]  = (float)(center[0] + fx + sx);
        out[7]  = (float)(center[1] + fy + sy);
        out[8]  = (float)(center[2] + fz + sz);
        out[9]  = (float)(center[0] + fx - sx);
        out[10] = (float)(center[1] + fy - sy);
        out[11] = (float)(center[2] + fz - sz);
        return out;
    }

    /**
     * 贴地的水平四边形（plan §1.3 {@code BongGroundDecalParticle}）。
     *
     * <p>法线锁定 +Y，所以 quad 落在水平面。{@code rotationRad} 绕 +Y 旋转贴图朝向
     * （用于符阵自转）。
     *
     * @param center        四边形中心（相对 camera）
     * @param halfSize      半边长（方形 decal）
     * @param rotationRad   绕 +Y 的旋转角（rad，CCW 为正）
     * @param yLift         向上微抬值（避免 z-fighting，典型 0.01~0.05）
     */
    public static float[] buildGroundDecalQuad(
        double[] center,
        double halfSize,
        double rotationRad,
        double yLift
    ) {
        double cos = Math.cos(rotationRad);
        double sin = Math.sin(rotationRad);
        // 本地四角（未旋转）：(-h,-h) (-h,+h) (+h,+h) (+h,-h)，在 XZ 平面
        double[][] local = {
            { -halfSize, -halfSize },
            { -halfSize,  halfSize },
            {  halfSize,  halfSize },
            {  halfSize, -halfSize },
        };
        float[] out = new float[12];
        double yBase = center[1] + yLift;
        for (int i = 0; i < 4; i++) {
            double lx = local[i][0];
            double lz = local[i][1];
            double x = cos * lx - sin * lz;
            double z = sin * lx + cos * lz;
            out[i * 3]     = (float)(center[0] + x);
            out[i * 3 + 1] = (float)yBase;
            out[i * 3 + 2] = (float)(center[2] + z);
        }
        return out;
    }

    /**
     * Selects the best terrain surface for {@code BongGroundDecalParticle}.
     *
     * <p>Candidates are absolute world-space top Y values sampled from current / below block shapes.
     * We choose the highest finite surface that is close to the particle origin: slightly above is
     * allowed for half slabs / carpets, but far-away ceilings and deep pits are ignored.
     */
    public static double fitGroundDecalY(double currentY, double[] candidateTopYs) {
        double best = Double.NEGATIVE_INFINITY;
        double minY = currentY - GROUND_DECAL_MAX_SNAP_DOWN;
        double maxY = currentY + GROUND_DECAL_MAX_SNAP_UP;
        for (double topY : candidateTopYs) {
            if (!Double.isFinite(topY) || topY < minY || topY > maxY) {
                continue;
            }
            best = Math.max(best, topY);
        }
        return Double.isFinite(best) ? best : currentY;
    }

    /**
     * Ribbon 段：给定两个环节（prev / curr）位置 + 指向 camera 的朝向辅助，
     * 构造一段连接前后两个位置的带宽四边形（plan §1.2 {@code BongRibbonParticle}）。
     *
     * <p>宽度方向 = ({@code curr - prev}) × ({@code curr - camera})，这样带子始终朝向 camera
     * 可见（弱 billboard），同时沿 ribbon 前进方向拉长。
     *
     * @param prev       前一环节位置（世界 - camera）
     * @param curr       当前环节位置（世界 - camera）
     * @param cameraToCurr 从 camera 指向 curr 的矢量（通常等于 curr 本身，camera-relative）
     * @param halfWidth  ribbon 半宽
     * @return 4 顶点 × 3 分量，顺序：prev-left, prev-right, curr-right, curr-left
     */
    public static float[] buildRibbonSegment(
        double[] prev,
        double[] curr,
        double[] cameraToCurr,
        double halfWidth
    ) {
        double dx = curr[0] - prev[0];
        double dy = curr[1] - prev[1];
        double dz = curr[2] - prev[2];
        double segLen = Math.sqrt(dx * dx + dy * dy + dz * dz);

        double[] width;
        if (segLen < EPSILON) {
            // 两节完全同点——退化；用 camera 的右向（cross(camToCurr, Y)）兜底
            width = cross3(cameraToCurr, UP_HINT);
            double wl = length3(width);
            if (wl < EPSILON) {
                width = new double[] { 1.0, 0.0, 0.0 };
                wl = 1.0;
            }
            width[0] /= wl; width[1] /= wl; width[2] /= wl;
        } else {
            double[] forward = { dx / segLen, dy / segLen, dz / segLen };
            width = cross3(forward, cameraToCurr);
            double wl = length3(width);
            if (wl < EPSILON) {
                // forward 与 cameraToCurr 平行：退回 forward × up
                width = cross3(forward, UP_HINT);
                wl = length3(width);
                if (wl < EPSILON) {
                    width = new double[] { 0.0, 0.0, 1.0 };
                    wl = 1.0;
                }
            }
            width[0] /= wl; width[1] /= wl; width[2] /= wl;
        }
        double sx = width[0] * halfWidth;
        double sy = width[1] * halfWidth;
        double sz = width[2] * halfWidth;

        float[] out = new float[12];
        out[0]  = (float)(prev[0] - sx);
        out[1]  = (float)(prev[1] - sy);
        out[2]  = (float)(prev[2] - sz);
        out[3]  = (float)(prev[0] + sx);
        out[4]  = (float)(prev[1] + sy);
        out[5]  = (float)(prev[2] + sz);
        out[6]  = (float)(curr[0] + sx);
        out[7]  = (float)(curr[1] + sy);
        out[8]  = (float)(curr[2] + sz);
        out[9]  = (float)(curr[0] - sx);
        out[10] = (float)(curr[1] - sy);
        out[11] = (float)(curr[2] - sz);
        return out;
    }

    // ---- tiny vec3 helpers (fresh arrays; tiny allocation cost OK at particle scale) ----

    static double length3(double[] v) {
        return Math.sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
    }

    static double[] cross3(double[] a, double[] b) {
        return new double[] {
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        };
    }
}
