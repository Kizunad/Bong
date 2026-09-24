package com.bong.client.visual.particle;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * {@link OrbitPath} / {@link SkillParticleSpawn.OrbitSpec} 的轨道数学 pin 测试
 * —— plan-skill-anim-fidelity-v1 §P5.1。
 *
 * <p>「环绕」在这里是可证的不变量而非形容词：半径恒定、角速度 = 线速度/半径、
 * 垂直漂移线性累积。{@link SkillParticlePlanTest} 断言各招<b>用了</b>环绕描述符，
 * 本文件断言环绕描述符<b>确实在绕</b>。
 */
public class OrbitPathTest {

    private static final double EPS = 1e-9;

    private static SkillParticleSpawn.OrbitSpec spec(
        double radius, double startAngle, double tangentialSpeed, double verticalSpeed) {
        return new SkillParticleSpawn.OrbitSpec(
            5.0, 70.0, -3.0, radius, startAngle, tangentialSpeed, verticalSpeed);
    }

    @Test
    void startsExactlyOnTheCircleAtStartAngle() {
        SkillParticleSpawn.OrbitSpec orbit = spec(0.55, 0.0, 0.08, 0.0);
        assertEquals(5.0 + 0.55, orbit.startX(), EPS, "起始角 0 应落在 +X 侧");
        assertEquals(70.0, orbit.startY(), EPS);
        assertEquals(-3.0, orbit.startZ(), EPS);

        SkillParticleSpawn.OrbitSpec quarter = spec(0.55, Math.PI / 2, 0.08, 0.0);
        assertEquals(5.0, quarter.startX(), 1e-12, "起始角 π/2 应落在 +Z 侧");
        assertEquals(-3.0 + 0.55, quarter.startZ(), EPS);
    }

    @Test
    void radiusIsInvariantAcrossManyTicks() {
        SkillParticleSpawn.OrbitSpec orbit = spec(0.55, 1.1, 0.08, 0.0);
        OrbitPath path = new OrbitPath(orbit);

        // 远超任何招式 lifetime，确认没有累积漂移。
        for (int tick = 0; tick < 600; tick++) {
            path.advance();
            assertEquals(0.55, path.horizontalRadius(), 1e-9,
                "第 " + tick + " tick 半径漂到 " + path.horizontalRadius());
        }
    }

    @Test
    void angularVelocityDerivesFromTangentialSpeedOverRadius() {
        SkillParticleSpawn.OrbitSpec orbit = spec(0.5, 0.0, 0.10, 0.0);
        assertEquals(0.2, orbit.angularVelocity(), EPS,
            "0.10 格/t 的线速度绕 0.5 格半径 = 0.2 rad/t");

        OrbitPath path = new OrbitPath(orbit);
        path.advance();
        assertEquals(0.2, path.angle(), EPS, "推进一 tick 应正好前进一个角速度");
        path.advance();
        assertEquals(0.4, path.angle(), EPS);
    }

    /** 同样线速度下，半径越小转得越快——这条保证「贴身环」不会看着比「大环」还慢。 */
    @Test
    void smallerRadiusSpinsFasterAtEqualTangentialSpeed() {
        double tight = spec(0.3, 0.0, 0.08, 0.0).angularVelocity();
        double wide = spec(0.9, 0.0, 0.08, 0.0).angularVelocity();
        assertTrue(tight > wide,
            "半径 0.3 的角速度 " + tight + " 应大于半径 0.9 的 " + wide);
    }

    @Test
    void verticalDriftAccumulatesLinearlyAndZeroMeansFlat() {
        OrbitPath rising = new OrbitPath(spec(0.55, 0.0, 0.08, 0.015));
        for (int tick = 1; tick <= 60; tick++) {
            rising.advance();
            assertEquals(70.0 + 0.015 * tick, rising.y(), 1e-9,
                "第 " + tick + " tick 的高度应线性累积");
        }
        assertEquals(60, rising.elapsedTicks());

        OrbitPath flat = new OrbitPath(spec(0.6, 0.0, 0.06, 0.0));
        for (int tick = 0; tick < 40; tick++) {
            flat.advance();
            assertEquals(70.0, flat.y(), EPS, "verticalSpeed=0 必须是严格水平环");
        }
    }

    /** 速度向量必须始终切于圆周（垂直于半径向量），否则线粒子朝向会与运动方向错位。 */
    @Test
    void velocityStaysTangentToTheCircle() {
        SkillParticleSpawn.OrbitSpec orbit = spec(0.5, 0.37, 0.10, 0.0);
        OrbitPath path = new OrbitPath(orbit);

        for (int tick = 0; tick < 50; tick++) {
            path.advance();
            double radialX = path.x() - orbit.centerX();
            double radialZ = path.z() - orbit.centerZ();
            double dot = radialX * path.velocityX() + radialZ * path.velocityZ();
            assertEquals(0.0, dot, 1e-9,
                "第 " + tick + " tick 速度与半径向量不正交（点积 " + dot + "）——不是切向");

            double speed = Math.hypot(path.velocityX(), path.velocityZ());
            assertEquals(0.10, speed, 1e-9, "切向速率应恒为 spec 的线速度");
        }
    }

    /** 正角速度 = 俯视顺时针（+x → +z）。三处环绕招共用这个方向约定。 */
    @Test
    void positiveSpeedRotatesFromPlusXTowardsPlusZ() {
        OrbitPath path = new OrbitPath(spec(1.0, 0.0, 0.10, 0.0));
        path.advance();
        assertTrue(path.z() > -3.0,
            "自 +X 起步、正线速度应朝 +Z 转，实际 z=" + path.z());
        assertTrue(path.x() < 5.0 + 1.0, "同时 X 应开始回落");
    }

    @Test
    void freshPathReportsStartStateBeforeAnyAdvance() {
        SkillParticleSpawn.OrbitSpec orbit = spec(0.55, 0.8, 0.08, 0.015);
        OrbitPath path = new OrbitPath(orbit);

        assertEquals(0, path.elapsedTicks());
        assertEquals(orbit.startAngle(), path.angle(), EPS);
        assertEquals(orbit.startX(), path.x(), EPS);
        assertEquals(orbit.startY(), path.y(), EPS, "未推进时不应已经产生垂直漂移");
        assertEquals(orbit.startZ(), path.z(), EPS);
    }

    @Test
    void degenerateOrbitParametersAreRejectedAtConstruction() {
        // 半径退化会让整圈粒子叠在圆心，视觉上等于「没有环」。
        assertThrows(IllegalArgumentException.class, () -> spec(0.0, 0.0, 0.08, 0.0),
            "半径 0 必须拒绝");
        assertThrows(IllegalArgumentException.class, () -> spec(-0.5, 0.0, 0.08, 0.0),
            "负半径必须拒绝");
        assertThrows(IllegalArgumentException.class, () -> spec(Double.NaN, 0.0, 0.08, 0.0),
            "NaN 半径必须拒绝");
        assertThrows(IllegalArgumentException.class,
            () -> spec(Double.POSITIVE_INFINITY, 0.0, 0.08, 0.0), "无穷半径必须拒绝");
        // 非有限的角度/速度会让坐标变 NaN，粒子静默消失。
        assertThrows(IllegalArgumentException.class, () -> spec(0.5, Double.NaN, 0.08, 0.0),
            "NaN 起始角必须拒绝");
        assertThrows(IllegalArgumentException.class, () -> spec(0.5, 0.0, Double.NaN, 0.0),
            "NaN 线速度必须拒绝");
        assertThrows(IllegalArgumentException.class, () -> spec(0.5, 0.0, 0.08, Double.NaN),
            "NaN 垂直速度必须拒绝");
    }

    /** 线速度为 0 是合法的「钉在圆周上的静止环」，不该被构造校验挡掉。 */
    @Test
    void zeroTangentialSpeedIsAStationaryRingNotAnError() {
        OrbitPath path = new OrbitPath(spec(0.5, 0.25, 0.0, 0.0));
        double x = path.x();
        double z = path.z();
        for (int tick = 0; tick < 20; tick++) {
            path.advance();
            assertEquals(x, path.x(), EPS, "线速度 0 时不该移动");
            assertEquals(z, path.z(), EPS);
        }
    }
}
