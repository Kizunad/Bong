package com.bong.client.animation;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** 下半身步态档位选择：覆盖优先级、每条边界与全部 enum 变体。 */
class GaitSelectorTest {
    private static GaitSelector.GaitInput input(
        boolean dashing, double speed, boolean sprinting, double horizontal, boolean onGround
    ) {
        return new GaitSelector.GaitInput(dashing, speed, sprinting, horizontal, onGround);
    }

    @Test
    void dashWinsOverEverythingIncludingAirborneAndIdle() {
        // dash 本就腾空且瞬间速度可能为 0，不能被 onGround / 静止判定吃掉
        assertEquals(
            GaitSelector.Gait.DASH,
            GaitSelector.select(input(true, 2.0, true, 0.0, false)),
            "DASHING 期望压过离地与静止判定，因为瞬步本身就是腾空动作"
        );
    }

    @Test
    void airborneWithoutDashPlaysNothing() {
        assertEquals(
            GaitSelector.Gait.NONE,
            GaitSelector.select(input(false, 1.0, false, 0.5, false)),
            "跳跃/坠落期间不该播步态循环"
        );
    }

    @Test
    void idleOnGroundPlaysNothing() {
        assertEquals(GaitSelector.Gait.NONE, GaitSelector.select(input(false, 1.0, false, 0.0, true)));
    }

    @Test
    void motionEpsilonIsTheWalkBoundary() {
        double eps = GaitSelector.MOTION_EPSILON;
        assertEquals(
            GaitSelector.Gait.NONE,
            GaitSelector.select(input(false, 1.0, false, eps - 1e-9, true)),
            "速度低于 MOTION_EPSILON 期望静止：抖动不该触发走路"
        );
        assertEquals(
            GaitSelector.Gait.WALK,
            GaitSelector.select(input(false, 1.0, false, eps, true)),
            "速度恰好等于 MOTION_EPSILON 期望 WALK：判定是严格小于才算静止"
        );
    }

    @Test
    void sprintThresholdIsStrictlyGreater() {
        double t = GaitSelector.SPRINT_SPEED_THRESHOLD;
        assertEquals(
            GaitSelector.Gait.WALK,
            GaitSelector.select(input(false, t, false, 0.2, true)),
            "倍率恰好等于阈值期望不升 SPRINT：判定是严格大于"
        );
        assertEquals(
            GaitSelector.Gait.SPRINT,
            GaitSelector.select(input(false, t + 1e-6, false, 0.2, true)),
            "倍率超过阈值期望 SPRINT，且不需要 vanilla sprinting"
        );
    }

    @Test
    void sprintOutranksVanillaSprinting() {
        assertEquals(
            GaitSelector.Gait.SPRINT,
            GaitSelector.select(input(false, 2.0, true, 0.3, true)),
            "灵气加速档期望压过 vanilla 疾跑档"
        );
    }

    @Test
    void vanillaSprintingIsJog() {
        assertEquals(GaitSelector.Gait.JOG, GaitSelector.select(input(false, 1.0, true, 0.25, true)));
    }

    @Test
    void plainMovementIsWalk() {
        assertEquals(GaitSelector.Gait.WALK, GaitSelector.select(input(false, 1.0, false, 0.12, true)));
    }

    @Test
    void nullInputIsNone() {
        assertEquals(GaitSelector.Gait.NONE, GaitSelector.select(null));
    }

    @Test
    void everyGaitVariantHasConsistentAssetMetadata() {
        assertNull(GaitSelector.Gait.NONE.animId(), "NONE 表示停掉本通道，不该有动画 id");
        assertFalse(GaitSelector.Gait.NONE.looped());
        for (GaitSelector.Gait gait : GaitSelector.Gait.values()) {
            if (gait == GaitSelector.Gait.NONE) {
                continue;
            }
            assertNotNull(gait.animId(), gait + " 必须有动画 id");
            assertEquals("bong", gait.animId().getNamespace(), gait + " 动画应在 bong 命名空间");
            assertTrue(
                gait.animId().getPath().startsWith("lower_"),
                gait + " 下半身动画 id 应以 lower_ 前缀标明分身层，实际 " + gait.animId()
            );
        }
        assertTrue(GaitSelector.Gait.WALK.looped(), "行走是循环步态");
        assertTrue(GaitSelector.Gait.JOG.looped());
        assertTrue(GaitSelector.Gait.SPRINT.looped());
        assertFalse(GaitSelector.Gait.DASH.looped(), "瞬步是一次性动画，播完自行结束");
    }

    @Test
    void dashUsesImmediateTransitionsAtServerBoundary() {
        assertEquals(0, GaitSelector.Gait.DASH.fadeInTicks(),
            "DASH 不应被默认淡入延迟，起始帧必须立即可见");
        assertEquals(0, GaitSelector.Gait.DASH.fadeOutTicks(),
            "DASH 在服务端 tick 4 结束时必须立即移除，不能残留淡出姿态");
    }
}
