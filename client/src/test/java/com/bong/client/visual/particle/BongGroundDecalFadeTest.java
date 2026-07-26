package com.bong.client.visual.particle;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 贴地 decal 的尾段淡出曲线（血渍"慢慢渗掉"靠它）。
 *
 * <p>MC 的 {@code SpriteBillboardParticle} 默认 alpha 恒定到死，decal 会在寿命末尾"啪"地
 * 整块消失。这里只测纯函数 {@code fadedAlpha}——粒子实例本身要 MC 运行时才建得起来。
 *
 * <p>默认 {@code fadeOutFrom = 1.0f}（不淡出）必须保持既有符阵 / 涟漪 decal 的表现不变，
 * 这是本次改动"不波及其他 decal"的锁。
 */
class BongGroundDecalFadeTest {

    private static final float EPS = 1e-6f;

    @Test
    void defaultDisabledFadeKeepsAlphaConstantForExistingDecals() {
        for (int age = 0; age <= 40; age++) {
            assertEquals(0.8f, BongGroundDecalParticle.fadedAlpha(age, 40, 0.8f, 1.0f), EPS,
                "fadeOutFrom = 1.0（默认）时 alpha 必须恒定——既有 decal 表现不允许被改动波及");
        }
    }

    @Test
    void alphaHoldsUntilFadeStartThenFallsToZero() {
        int maxAge = 100;
        float base = 0.9f;
        float from = 0.6f;
        assertEquals(base, BongGroundDecalParticle.fadedAlpha(0, maxAge, base, from), EPS);
        assertEquals(base, BongGroundDecalParticle.fadedAlpha(60, maxAge, base, from), EPS,
            "刚好到淡出起点时还是满 alpha");
        assertEquals(base * 0.5f, BongGroundDecalParticle.fadedAlpha(80, maxAge, base, from), 1e-5f,
            "淡出区间过半时应为一半 alpha（线性）");
        assertEquals(0.0f, BongGroundDecalParticle.fadedAlpha(100, maxAge, base, from), EPS,
            "寿命耗尽时应完全透明，而不是带着可见 alpha 突然消失");
    }

    @Test
    void fadeIsMonotonicallyNonIncreasing() {
        float previous = Float.MAX_VALUE;
        for (int age = 0; age <= 120; age++) {
            float alpha = BongGroundDecalParticle.fadedAlpha(age, 100, 0.75f, 0.45f);
            assertTrue(alpha <= previous + EPS,
                "alpha 在 age=" + age + " 处回升了（" + previous + " → " + alpha + "）——血渍不能越渗越浓");
            assertTrue(alpha >= 0.0f && alpha <= 0.75f, "alpha 越界：" + alpha);
            previous = alpha;
        }
    }

    @Test
    void degenerateInputsDoNotProduceNaNOrNegativeAlpha() {
        assertEquals(0.5f, BongGroundDecalParticle.fadedAlpha(3, 0, 0.5f, 0.5f), EPS,
            "maxAge = 0 时不得除零，原样返回基准 alpha");
        assertEquals(0.5f, BongGroundDecalParticle.fadedAlpha(3, -10, 0.5f, 0.5f), EPS,
            "负 maxAge 同样按不淡出处理");
        assertEquals(0.0f, BongGroundDecalParticle.fadedAlpha(200, 100, 0.5f, 0.0f), EPS,
            "fadeOutFrom = 0（全程淡出）且已超龄时应为 0");
        assertEquals(0.5f, BongGroundDecalParticle.fadedAlpha(-5, 100, 0.5f, 0.2f), EPS,
            "负 age（不该出现，但别炸）按未开始淡出处理");
        assertEquals(0.0f, BongGroundDecalParticle.fadedAlpha(50, 100, 0.0f, 0.2f), EPS,
            "基准 alpha 为 0 时恒为 0");
    }

    @Test
    void outOfRangeFadeStartStillProducesUsableAlpha() {
        // setter 会把比例钳到 [0,1]（构造 MC 粒子实例需要运行时，这里直接验纯函数在
        // 越界输入下也不产出 NaN / 负值 / >base 的 alpha）。
        for (float from : new float[] { -2.0f, -0.1f, 0.0f, 0.5f, 1.0f, 3.0f }) {
            for (int age : new int[] { 0, 25, 50, 99, 100, 250 }) {
                float alpha = BongGroundDecalParticle.fadedAlpha(age, 100, 0.6f, from);
                assertTrue(Float.isFinite(alpha) && alpha >= 0.0f && alpha <= 0.6f,
                    "fadeOutFrom=" + from + " age=" + age + " 产出了非法 alpha " + alpha);
            }
        }
    }
}
