package com.bong.client.visual.particle;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 纯几何单测（plan-particle-system-v1 §5.2 "§1 三个基类…单元测试渲染"）。
 *
 * <p>只验证顶点位置是否合理——MC 的 {@code VertexConsumer} / {@code Camera} 无法在 JVM
 * 单测环境实例化，所以 {@code BongLineParticle.buildGeometry} 整体只能在 {@code runClient} 下验证。
 * 这里验证的 "几何算对"已经能拦截 90% 的 regression（方向反了、长度错了、宽轴垂直度差）。
 */
public class BongParticleGeometryTest {

    private static final double EPS = 1e-5;

    // ---------- buildLineQuad ----------

    @Test
    void lineQuadAlignsWithVelocity() {
        double[] center = { 0, 0, 0 };
        double[] velocity = { 10.0, 0.0, 0.0 };
        float[] quad = BongParticleGeometry.buildLineQuad(center, velocity, 1.0, 0.0, 0.1);

        // 速度向 +X 方向：前端 2 顶点的 x > 0，后端 2 顶点的 x < 0
        assertTrue(quad[0] < 0, "tail vertex 0 should be -X");
        assertTrue(quad[3] < 0, "tail vertex 1 should be -X");
        assertTrue(quad[6] > 0, "head vertex 2 should be +X");
        assertTrue(quad[9] > 0, "head vertex 3 should be +X");

        // 长度 = |v| * factor = 10 * 1 = 10；头尾距 = length
        double headX = quad[6];
        double tailX = quad[0];
        assertEquals(10.0, headX - tailX, EPS);
    }

    @Test
    void lineQuadLengthFactorMultiplies() {
        float[] q1 = BongParticleGeometry.buildLineQuad(
            new double[]{0,0,0}, new double[]{1,0,0}, 1.0, 0.0, 0.1);
        float[] q2 = BongParticleGeometry.buildLineQuad(
            new double[]{0,0,0}, new double[]{1,0,0}, 3.0, 0.0, 0.1);
        double len1 = q1[6] - q1[0];
        double len2 = q2[6] - q2[0];
        assertEquals(len1 * 3, len2, EPS);
    }

    @Test
    void lineQuadFallsBackToMinLengthForStaticParticle() {
        // velocity = 0 向量：应使用 minLength 而非 0
        float[] quad = BongParticleGeometry.buildLineQuad(
            new double[]{0,0,0}, new double[]{0,0,0}, 1.0, 2.0, 0.1);
        double len = quad[6] - quad[0];
        assertEquals(2.0, len, EPS, "zero velocity should fall back to minLength");
    }

    @Test
    void lineQuadHalfWidthApplies() {
        float[] quad = BongParticleGeometry.buildLineQuad(
            new double[]{0,0,0}, new double[]{1,0,0}, 2.0, 0.0, 0.5);
        // width axis is cross(+X, +Y) = -Z，half-width 0.5 → 顶点的 z 跨度 = 1.0
        double zSpan = Math.abs(quad[5] - quad[2]); // vertex 0 vs 1
        assertEquals(1.0, zSpan, EPS);
    }

    @Test
    void lineQuadHandlesVerticalVelocity() {
        // velocity = +Y：forward 平行 UP_HINT，需要走 +Z 回退路径，不应出现 NaN
        float[] quad = BongParticleGeometry.buildLineQuad(
            new double[]{0,0,0}, new double[]{0,10,0}, 1.0, 0.0, 0.1);
        for (float f : quad) {
            assertTrue(Float.isFinite(f), "all vertices must be finite: " + f);
        }
        // 长度仍然是 10
        double yLen = quad[7] - quad[1];
        assertEquals(10.0, yLen, EPS);
    }

    // ---------- buildGroundDecalQuad ----------

    @Test
    void groundDecalIsHorizontalAndLifted() {
        double yCenter = 64.0;
        double lift = 0.02;
        float[] quad = BongParticleGeometry.buildGroundDecalQuad(
            new double[]{0, yCenter, 0}, 1.0, 0.0, lift);
        // 所有 4 顶点 y 一致 = yCenter + lift
        assertEquals((float)(yCenter + lift), quad[1], EPS);
        assertEquals((float)(yCenter + lift), quad[4], EPS);
        assertEquals((float)(yCenter + lift), quad[7], EPS);
        assertEquals((float)(yCenter + lift), quad[10], EPS);
    }

    @Test
    void groundDecalRotationRotatesInXZ() {
        // 未旋转：四角 x 分量为 ±halfSize。旋转 90°（π/2）后，原 +X 角应到 +Z 轴
        float[] zero = BongParticleGeometry.buildGroundDecalQuad(
            new double[]{0, 0, 0}, 1.0, 0.0, 0.0);
        float[] rot90 = BongParticleGeometry.buildGroundDecalQuad(
            new double[]{0, 0, 0}, 1.0, Math.PI / 2, 0.0);

        // 未旋转顶点 2 (halfSize, halfSize) → (x=1, z=1)
        assertEquals(1f, zero[6], EPS);
        assertEquals(1f, zero[8], EPS);
        // 旋转 90° 后同一顶点 → (x=-1, z=1)
        assertEquals(-1f, rot90[6], EPS);
        assertEquals(1f,  rot90[8], EPS);
    }

    @Test
    void groundDecalSizeControlsHalfSize() {
        float[] quad = BongParticleGeometry.buildGroundDecalQuad(
            new double[]{0, 0, 0}, 2.5, 0.0, 0.0);
        // 最大 x / z 应该都是 ±2.5
        float maxX = Math.max(Math.max(quad[0], quad[3]), Math.max(quad[6], quad[9]));
        float maxZ = Math.max(Math.max(quad[2], quad[5]), Math.max(quad[8], quad[11]));
        assertEquals(2.5f, maxX, EPS);
        assertEquals(2.5f, maxZ, EPS);
    }

    // ---------- buildRibbonSegment ----------

    @Test
    void ribbonSegmentConnectsPrevAndCurr() {
        // prev 在原点，curr 向 +X 1m；camera 从 -Z 方向看（即 curr 相对 camera 沿 +Z 方向某处）
        double[] prev = { 0, 0, 0 };
        double[] curr = { 1, 0, 0 };
        double[] camToCurr = { 1, 0, 5 };
        float[] quad = BongParticleGeometry.buildRibbonSegment(prev, curr, camToCurr, 0.1);

        // 顶点 0/1 在 prev 附近（x ≈ 0），顶点 2/3 在 curr 附近（x ≈ 1）
        assertEquals(0f, quad[0], EPS);
        assertEquals(0f, quad[3], EPS);
        assertEquals(1f, quad[6], EPS);
        assertEquals(1f, quad[9], EPS);
    }

    @Test
    void ribbonSegmentHalfWidthAppliesAlongWidthAxis() {
        double[] prev = { 0, 0, 0 };
        double[] curr = { 1, 0, 0 };
        double[] camToCurr = { 0, 0, 1 };
        float[] quad = BongParticleGeometry.buildRibbonSegment(prev, curr, camToCurr, 0.5);

        // forward=+X, camToCurr=+Z → width = +X × +Z = -Y；半宽 0.5 → y 跨度 = 1
        double ySpanAtPrev = Math.abs(quad[4] - quad[1]);
        double ySpanAtCurr = Math.abs(quad[10] - quad[7]);
        assertEquals(1.0, ySpanAtPrev, EPS);
        assertEquals(1.0, ySpanAtCurr, EPS);
    }

    @Test
    void ribbonSegmentDegenerateSegmentDoesNotProduceNaN() {
        // prev == curr：forward 无法算出，退化路径
        double[] prev = { 5, 10, 15 };
        double[] curr = { 5, 10, 15 };
        double[] camToCurr = { 0, 0, 10 };
        float[] quad = BongParticleGeometry.buildRibbonSegment(prev, curr, camToCurr, 0.1);
        for (float f : quad) {
            assertTrue(Float.isFinite(f), "degenerate ribbon vertex must be finite: " + f);
        }
    }

    @Test
    void lineQuadVerticesAreDistinctForNonZeroInputs() {
        // 冒烟：确保 4 顶点两两不完全重合（防止未来 regression 把 quad 压扁）
        float[] quad = BongParticleGeometry.buildLineQuad(
            new double[]{0,0,0}, new double[]{1,0,0}, 1.0, 0.0, 0.1);
        float[] v0 = { quad[0], quad[1], quad[2] };
        float[] v1 = { quad[3], quad[4], quad[5] };
        float[] v2 = { quad[6], quad[7], quad[8] };
        float[] v3 = { quad[9], quad[10], quad[11] };
        // assertNotArrayEquals 不是 JUnit 常规 API，用否定 assertArrayEquals 的思路：
        org.junit.jupiter.api.Assertions.assertThrows(
            AssertionError.class,
            () -> assertArrayEquals(v0, v2, EPS_FLOAT)
        );
        org.junit.jupiter.api.Assertions.assertThrows(
            AssertionError.class,
            () -> assertArrayEquals(v0, v1, EPS_FLOAT)
        );
        org.junit.jupiter.api.Assertions.assertThrows(
            AssertionError.class,
            () -> assertArrayEquals(v0, v3, EPS_FLOAT)
        );
    }

    private static final float EPS_FLOAT = 1e-5f;
}
