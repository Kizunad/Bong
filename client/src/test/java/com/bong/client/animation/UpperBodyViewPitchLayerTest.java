package com.bong.client.animation;

import dev.kosmx.playerAnim.api.TransformType;
import dev.kosmx.playerAnim.api.layered.AnimationStack;
import dev.kosmx.playerAnim.core.data.AnimationFormat;
import dev.kosmx.playerAnim.core.data.KeyframeAnimation;
import dev.kosmx.playerAnim.core.util.Vec3f;
import org.junit.jupiter.api.Test;

import java.util.UUID;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** 上半身视角跟随层：分档、钳位、平滑、以及"只碰 torso 的 BEND"这条分身契约。 */
class UpperBodyViewPitchLayerTest {
    private static UpperBodyViewPitchLayer layer(double pitchDeg, boolean armed) {
        return new UpperBodyViewPitchLayer(() -> pitchDeg, () -> armed);
    }

    private static UpperBodyViewPitchLayer settled(double pitchDeg, boolean armed) {
        UpperBodyViewPitchLayer l = layer(pitchDeg, armed);
        for (int i = 0; i < 60; i++) {
            l.tick();
        }
        return l;
    }

    private static UpperBodyViewPitchLayer suppressedLayer(double pitchDeg, boolean armed) {
        return new UpperBodyViewPitchLayer(() -> pitchDeg, () -> armed, () -> true);
    }

    @Test
    void casualBandIsSmallerThanArmedBand() {
        assertEquals(
            UpperBodyViewPitchLayer.CASUAL_MAX_DEG, layer(90.0, false).targetDeg(), 1e-4,
            "低头到底、常态档期望只到 CASUAL_MAX_DEG"
        );
        assertEquals(
            UpperBodyViewPitchLayer.ARMED_MAX_DEG, layer(90.0, true).targetDeg(), 1e-4,
            "低头到底、持械档期望放开到 ARMED_MAX_DEG"
        );
        assertTrue(
            UpperBodyViewPitchLayer.CASUAL_MAX_DEG < UpperBodyViewPitchLayer.ARMED_MAX_DEG,
            "常态幅度必须小于持械幅度，否则分档没有意义"
        );
    }

    @Test
    void lookingUpBendsTheOtherWay() {
        assertEquals(
            -UpperBodyViewPitchLayer.CASUAL_MAX_DEG, layer(-90.0, false).targetDeg(), 1e-4,
            "抬头期望反向折弯（挺背），符号与低头相反"
        );
    }

    @Test
    void levelViewMeansNoBend() {
        assertEquals(0.0f, layer(0.0, true).targetDeg(), 1e-6);
    }

    @Test
    void pitchIsClampedBeyondVanillaRange() {
        assertEquals(
            UpperBodyViewPitchLayer.CASUAL_MAX_DEG, layer(400.0, false).targetDeg(), 1e-4,
            "越界 pitch 期望钳到档位上限而不是线性放大"
        );
        assertEquals(-UpperBodyViewPitchLayer.CASUAL_MAX_DEG, layer(-400.0, false).targetDeg(), 1e-4);
    }

    @Test
    void smoothingApproachesTargetWithoutOvershoot() {
        UpperBodyViewPitchLayer l = layer(90.0, false);
        float target = UpperBodyViewPitchLayer.CASUAL_MAX_DEG;
        float previous = 0.0f;
        for (int i = 0; i < 12; i++) {
            l.tick();
            float now = l.currentDegForTest();
            assertTrue(now >= previous, "第 " + i + " tick 期望单调逼近目标，实际回退到 " + now);
            assertTrue(now <= target + 1e-4, "期望不过冲目标 " + target + "，实际 " + now);
            previous = now;
        }
        assertEquals(target, previous, 0.5, "12 tick 后应基本到位");
    }

    @Test
    void nonTorsoPartsArePassedThroughUntouched() {
        UpperBodyViewPitchLayer l = settled(90.0, true);
        Vec3f in = new Vec3f(0.1f, 0.2f, 0.3f);
        for (String part : new String[] {"rightArm", "leftArm", "head", "body", "leftLeg", "rightLeg"}) {
            assertSame(in, l.get3DTransform(part, TransformType.BEND, 1.0f, in),
                part + " 不属于本层职责，期望原样透传（分身契约）");
        }
    }

    @Test
    void nonBendTransformsArePassedThroughUntouched() {
        UpperBodyViewPitchLayer l = settled(90.0, true);
        Vec3f in = new Vec3f(0.1f, 0.2f, 0.3f);
        assertSame(in, l.get3DTransform("torso", TransformType.ROTATION, 1.0f, in),
            "本层只做 bend，不该动 torso 的旋转——那是招式动画的地盘");
        assertSame(in, l.get3DTransform("torso", TransformType.POSITION, 1.0f, in));
    }

    @Test
    void torsoBendAddsOnTopOfLowerLayerValue() {
        UpperBodyViewPitchLayer l = settled(90.0, true);
        float own = l.bendRadians(1.0f);
        Vec3f lower = new Vec3f(0.5f, 0.25f, 0.0f);   // 下层已有 (bendAxis, bendValue)
        Vec3f out = l.get3DTransform("torso", TransformType.BEND, 1.0f, lower);
        assertEquals(0.25f + own, out.getY(), 1e-5,
            "期望在下层 bendValue 之上叠加本层量，而不是覆盖");
        assertEquals(0.5f, out.getX(), 1e-6, "下层已声明 bendAxis 时应保留它，不抢方向");
    }

    @Test
    void zeroBendKeepsLowerLayerBendAxisIntact() {
        UpperBodyViewPitchLayer l = settled(0.0, false);   // 视角水平 → 本层无贡献
        Vec3f lower = new Vec3f(1.234f, 0.0f, 0.0f);
        assertSame(lower, l.get3DTransform("torso", TransformType.BEND, 1.0f, lower),
            "本层无可见折弯时必须完全透传，否则会把下层的 bendAxis 抹成 0");
    }

    @Test
    void renderInterpolatesBetweenPreviousAndCurrentTick() {
        UpperBodyViewPitchLayer l = layer(90.0, true);
        l.tick();                       // previous=0, current=第一步
        float atStart = l.bendRadians(0.0f);
        float atEnd = l.bendRadians(1.0f);
        assertEquals(0.0f, atStart, 1e-6, "tickDelta=0 期望取上一 tick 的值");
        assertTrue(atEnd > atStart, "tickDelta=1 期望取当前 tick 的值，渲染帧间应连续过渡");
        float mid = l.bendRadians(0.5f);
        assertEquals((atStart + atEnd) / 2.0f, mid, 1e-5, "中间 tickDelta 期望线性插值");
    }

    @Test
    void activeJianAnimationSuppressesViewPitchThroughProductionBridge() {
        BongAnimationPlayer.resetForTest();
        AnimationLayerManager.resetForTest();
        UUID playerId = UUID.randomUUID();
        AnimationStack stack = new AnimationStack();
        BongAnimationRegistry.register(BongAnimations.JIAN_DRAW_WAIST, minimalAnimation());
        assertTrue(AnimationLayerManager.playOnStack(
            stack,
            playerId,
            AnimationLayerManager.Channel.UPPER_BODY,
            BongAnimations.JIAN_DRAW_WAIST,
            0,
            0
        ));

        UpperBodyViewPitchLayer l = new UpperBodyViewPitchLayer(
            () -> 90.0,
            () -> true,
            () -> AnimationLayerManager.isJianAnimationActive(playerId, stack)
        );
        Vec3f in = new Vec3f(0.5f, 0.25f, 0.0f);
        l.tick();

        assertTrue(AnimationLayerManager.isJianAnimationActive(playerId, stack),
            "真实 Jian producer/manager bridge 必须报告当前 stack 上有 Jian 动画");
        assertEquals(0.0f, l.targetDeg(), 1e-6f,
            "Jian 招式活跃时视角目标必须归零，避免继续驱动躯干");
        assertSame(in, l.get3DTransform("torso", TransformType.BEND, 1.0f, in),
            "Jian 招式活跃时必须直接透传已有 bend，不能与视角层叠加");

        assertTrue(AnimationLayerManager.stopOnStack(
            stack, playerId, AnimationLayerManager.Channel.UPPER_BODY, 0
        ));
        assertFalse(AnimationLayerManager.isJianAnimationActive(playerId, stack),
            "Jian 停止后 production suppression bridge 必须恢复为 false");
        assertTrue(l.targetDeg() > 0.0f,
            "Jian 停止后视角层必须恢复目标折弯");
        AnimationLayerManager.resetForTest();
        BongAnimationPlayer.resetForTest();
    }

    private static KeyframeAnimation minimalAnimation() {
        KeyframeAnimation.AnimationBuilder builder =
            new KeyframeAnimation.AnimationBuilder(AnimationFormat.UNKNOWN);
        builder.endTick = 1;
        builder.isLooped = false;
        return builder.build();
    }

    @Test
    void suppressedJianTakeoverPassesExistingTorsoBendThroughImmediately() {
        UpperBodyViewPitchLayer l = suppressedLayer(90.0, true);
        Vec3f in = new Vec3f(0.5f, 0.25f, 0.0f);
        l.tick();
        assertEquals(0.0f, l.targetDeg(), 1e-6f,
            "Jian 招式活跃时视角目标必须归零，避免继续驱动躯干");
        assertSame(in, l.get3DTransform("torso", TransformType.BEND, 1.0f, in),
            "Jian 招式活跃时必须直接透传已有 bend，不能与视角层叠加");
    }

    @Test
    void prioritySitsBetweenLowerBodyAndUpperBodyChannels() {
        assertTrue(
            UpperBodyViewPitchLayer.PRIORITY > AnimationLayerManager.Channel.LOWER_BODY.priority(),
            "视角跟随必须高于下半身步态，否则步态层会把 torso 压回去"
        );
        assertTrue(
            UpperBodyViewPitchLayer.PRIORITY < AnimationLayerManager.Channel.UPPER_BODY.priority(),
            "视角跟随必须低于招式动画，否则出招时躯干还被视角牵着走"
        );
    }
}
