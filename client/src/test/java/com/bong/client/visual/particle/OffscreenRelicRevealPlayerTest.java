package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import org.junit.jupiter.api.Test;

import java.util.Optional;
import java.util.OptionalInt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-offscreen-war-v1 P3 二修（CodeRabbit Major coverage）：headless 锁住
 * {@link OffscreenRelicRevealPlayer#resolveDecalSpec} 的全部「payload -> 渲染标量」规范化
 * 分支。该纯函数是 CodeRabbit #3 加固（strength 先 {@link Double#isFinite} 再 clamp[0,1]）的
 * 回归面所在——把它从 {@code play()} 抽出来后，NaN / 负 / 超大 strength → halfSize/alpha、
 * durationTicks 越界 → maxAge 这些算术全部可在无 {@code MinecraftClient} 的 JVM 里断言。
 *
 * <p>{@code play()} 里剩下的 {@code world == null} 早退 / no-sprite / 提交 particleManager 等
 * 渲染副作用需要已初始化的 MinecraftClient，留在人工 runClient checklist（与 e2e「玩家靠近
 * 物化」同一可测性边界）；本类不碰它们。
 */
class OffscreenRelicRevealPlayerTest {

    // 与被测常量手动对齐（被测里是 private；这里复刻数值，改动任一端都会让断言撞红，是有意的
    // pin——渲染规格变了测试必须一起改）。
    private static final double HALF_SIZE_BASE = 0.55;
    private static final double HALF_SIZE_PER_STRENGTH = 0.30;
    private static final float ALPHA_MIN = 0.30f;
    private static final float ALPHA_MAX = 0.80f;
    private static final int MAX_AGE_MIN = 60;
    private static final int MAX_AGE_MAX = 200;
    private static final int DEFAULT_MAX_AGE = 200;
    private static final double DEFAULT_STRENGTH = 0.7;
    private static final int FALLBACK_RGB = 0xB8AFA0;

    private static final double EPS = 1e-6;

    private static VfxEventPayload.SpawnParticle payload(
        OptionalInt colorRgb,
        Optional<Double> strength,
        OptionalInt durationTicks
    ) {
        return new VfxEventPayload.SpawnParticle(
            OffscreenRelicRevealPlayer.EVENT_ID,
            new double[] { 12.0, 64.0, -8.0 },
            Optional.empty(),
            colorRgb,
            strength,
            OptionalInt.of(1), // count（burst-once）：resolveDecalSpec 不读它，给个真实值
            durationTicks
        );
    }

    private static float channel(int rgb, int shift) {
        return ((rgb >> shift) & 0xFF) / 255f;
    }

    @Test
    void happyPathResolvesColorSizeAlphaLifetimeFromPayload() {
        // 典型 relic payload：双色之一（骨堆灰白）+ strength 0.7 + lifetime 拉满 200。
        int rgb = 0xB8AFA0;
        OffscreenRelicRevealPlayer.DecalSpec spec = OffscreenRelicRevealPlayer.resolveDecalSpec(
            payload(OptionalInt.of(rgb), Optional.of(0.7), OptionalInt.of(200))
        );

        assertEquals(channel(rgb, 16), spec.red(), 1e-4f,
            "red channel must come straight from payload.color high byte");
        assertEquals(channel(rgb, 8), spec.green(), 1e-4f,
            "green channel must come straight from payload.color mid byte");
        assertEquals(channel(rgb, 0), spec.blue(), 1e-4f,
            "blue channel must come straight from payload.color low byte");
        assertEquals(HALF_SIZE_BASE + HALF_SIZE_PER_STRENGTH * 0.7, spec.halfSize(), EPS,
            "halfSize must be BASE + PER_STRENGTH*strength for an in-range strength=0.7");
        assertEquals(0.7f, spec.alpha(), 1e-4f,
            "alpha must equal strength when it sits inside [ALPHA_MIN, ALPHA_MAX]; 0.7 is in range");
        assertEquals(200, spec.maxAge(),
            "maxAge must pass through unchanged when durationTicks=200 equals the upper clamp bound");
    }

    @Test
    void emptyPayloadFallsBackToDefaults() {
        // 服务端只发坐标、不带 color/strength/duration（合法最小 payload）→ 全部走兜底。
        OffscreenRelicRevealPlayer.DecalSpec spec = OffscreenRelicRevealPlayer.resolveDecalSpec(
            payload(OptionalInt.empty(), Optional.empty(), OptionalInt.empty())
        );

        assertEquals(channel(FALLBACK_RGB, 16), spec.red(), 1e-4f,
            "missing color must fall back to FALLBACK_RGB (#B8AFA0 bone) — red byte");
        assertEquals(channel(FALLBACK_RGB, 8), spec.green(), 1e-4f,
            "missing color must fall back to FALLBACK_RGB — green byte");
        assertEquals(channel(FALLBACK_RGB, 0), spec.blue(), 1e-4f,
            "missing color must fall back to FALLBACK_RGB — blue byte");
        assertEquals(HALF_SIZE_BASE + HALF_SIZE_PER_STRENGTH * DEFAULT_STRENGTH, spec.halfSize(), EPS,
            "missing strength must use DEFAULT_STRENGTH=0.7 for halfSize");
        assertEquals((float) DEFAULT_STRENGTH, spec.alpha(), 1e-4f,
            "missing strength must use DEFAULT_STRENGTH=0.7 for alpha (0.7 is within the alpha clamp)");
        assertEquals(DEFAULT_MAX_AGE, spec.maxAge(),
            "missing durationTicks must use DEFAULT_MAX_AGE=200");
    }

    @Test
    void strengthZeroGivesMinSizeAndAlphaFloor() {
        // 边界下端 strength=0.0：halfSize 落到 BASE；alpha 被 ALPHA_MIN 兜住（0.0 < 0.30）。
        OffscreenRelicRevealPlayer.DecalSpec spec = OffscreenRelicRevealPlayer.resolveDecalSpec(
            payload(OptionalInt.of(0x000000), Optional.of(0.0), OptionalInt.of(120))
        );

        assertEquals(HALF_SIZE_BASE, spec.halfSize(), EPS,
            "strength=0 must yield the base halfSize (no per-strength growth)");
        assertEquals(ALPHA_MIN, spec.alpha(), 1e-4f,
            "strength=0 is below ALPHA_MIN=0.30 so alpha must be clamped up to the floor, not 0");
    }

    @Test
    void strengthOneGivesMaxSizeAndAlphaCap() {
        // 边界上端 strength=1.0：halfSize=BASE+PER；alpha 被 ALPHA_MAX 封顶（1.0 > 0.80）。
        OffscreenRelicRevealPlayer.DecalSpec spec = OffscreenRelicRevealPlayer.resolveDecalSpec(
            payload(OptionalInt.of(0xFFFFFF), Optional.of(1.0), OptionalInt.of(120))
        );

        assertEquals(HALF_SIZE_BASE + HALF_SIZE_PER_STRENGTH, spec.halfSize(), EPS,
            "strength=1 must yield the maximum halfSize (full per-strength growth)");
        assertEquals(ALPHA_MAX, spec.alpha(), 1e-4f,
            "strength=1 is above ALPHA_MAX=0.80 so alpha must be clamped down to the cap, not 1");
    }

    @Test
    void midStrengthBelowAlphaFloorStillFloorsAlphaButGrowsSize() {
        // 关键 decoupling 用例：strength=0.1 时 halfSize 已随 strength 增长（0.58），
        // 但 alpha 仍被 ALPHA_MIN 兜到 0.30 —— 证明 alpha 下限独立于 size，不是同一个量。
        double strength = 0.1;
        OffscreenRelicRevealPlayer.DecalSpec spec = OffscreenRelicRevealPlayer.resolveDecalSpec(
            payload(OptionalInt.of(0x102030), Optional.of(strength), OptionalInt.of(100))
        );

        assertEquals(HALF_SIZE_BASE + HALF_SIZE_PER_STRENGTH * strength, spec.halfSize(), EPS,
            "halfSize must track strength=0.1 (BASE + 0.30*0.1 = 0.58) even though alpha gets floored");
        assertEquals(ALPHA_MIN, spec.alpha(), 1e-4f,
            "alpha must be floored to ALPHA_MIN at strength=0.1 (below floor) while halfSize keeps growing");
    }

    @Test
    void midStrengthAboveAlphaCapStillCapsAlphaButGrowsSize() {
        // 对称用例：strength=0.9 时 halfSize=0.82（继续长），alpha 被 ALPHA_MAX 封到 0.80。
        double strength = 0.9;
        OffscreenRelicRevealPlayer.DecalSpec spec = OffscreenRelicRevealPlayer.resolveDecalSpec(
            payload(OptionalInt.of(0x102030), Optional.of(strength), OptionalInt.of(100))
        );

        assertEquals(HALF_SIZE_BASE + HALF_SIZE_PER_STRENGTH * strength, spec.halfSize(), EPS,
            "halfSize must track strength=0.9 (BASE + 0.30*0.9 = 0.82) even though alpha gets capped");
        assertEquals(ALPHA_MAX, spec.alpha(), 1e-4f,
            "alpha must be capped to ALPHA_MAX at strength=0.9 (above cap) while halfSize keeps growing");
    }

    @Test
    void negativeStrengthClampsToZero() {
        // 非法负 strength（off-by-one below 下界）：clamp→0 → 最小 halfSize + alpha floor。
        OffscreenRelicRevealPlayer.DecalSpec spec = OffscreenRelicRevealPlayer.resolveDecalSpec(
            payload(OptionalInt.of(0x102030), Optional.of(-5.0), OptionalInt.of(100))
        );

        assertEquals(HALF_SIZE_BASE, spec.halfSize(), EPS,
            "negative strength must clamp to 0 so halfSize never goes below BASE (no negative decal)");
        assertEquals(ALPHA_MIN, spec.alpha(), 1e-4f,
            "negative strength must clamp to 0 then floor alpha to ALPHA_MIN");
    }

    @Test
    void oversizedStrengthClampsToOne() {
        // 非法超大 strength（off-by-one above 上界）：clamp→1 → 最大 halfSize + alpha cap，
        // 绝不算出超大贴花（CodeRabbit #3 加固的核心）。
        OffscreenRelicRevealPlayer.DecalSpec spec = OffscreenRelicRevealPlayer.resolveDecalSpec(
            payload(OptionalInt.of(0x102030), Optional.of(9.0), OptionalInt.of(100))
        );

        assertEquals(HALF_SIZE_BASE + HALF_SIZE_PER_STRENGTH, spec.halfSize(), EPS,
            "oversized strength (9.0) must clamp to 1 so halfSize caps at BASE+PER, never blows up");
        assertEquals(ALPHA_MAX, spec.alpha(), 1e-4f,
            "oversized strength must clamp to 1 then cap alpha at ALPHA_MAX");
    }

    @Test
    void nanStrengthFallsBackToDefaultNotPropagated() {
        // NaN strength：必须退回 DEFAULT_STRENGTH，绝不让 NaN 污染 halfSize/alpha（否则贴花
        // 尺寸/透明度变 NaN，渲染崩坏）。这是 isFinite 守卫存在的理由。
        OffscreenRelicRevealPlayer.DecalSpec spec = OffscreenRelicRevealPlayer.resolveDecalSpec(
            payload(OptionalInt.of(0x102030), Optional.of(Double.NaN), OptionalInt.of(100))
        );

        assertTrue(Double.isFinite(spec.halfSize()),
            "halfSize must stay finite when strength is NaN (isFinite guard falls back to DEFAULT_STRENGTH)");
        assertTrue(Float.isFinite(spec.alpha()),
            "alpha must stay finite when strength is NaN");
        assertEquals(HALF_SIZE_BASE + HALF_SIZE_PER_STRENGTH * DEFAULT_STRENGTH, spec.halfSize(), EPS,
            "NaN strength must resolve halfSize via DEFAULT_STRENGTH=0.7, not NaN");
        assertEquals((float) DEFAULT_STRENGTH, spec.alpha(), 1e-4f,
            "NaN strength must resolve alpha via DEFAULT_STRENGTH=0.7, not NaN");
    }

    @Test
    void positiveInfinityStrengthFallsBackToDefault() {
        // +Inf：isFinite 失败 → DEFAULT_STRENGTH（绝不算出无穷大的贴花尺寸）。
        OffscreenRelicRevealPlayer.DecalSpec spec = OffscreenRelicRevealPlayer.resolveDecalSpec(
            payload(OptionalInt.of(0x102030), Optional.of(Double.POSITIVE_INFINITY), OptionalInt.of(100))
        );

        assertEquals(HALF_SIZE_BASE + HALF_SIZE_PER_STRENGTH * DEFAULT_STRENGTH, spec.halfSize(), EPS,
            "+Infinity strength must fall back to DEFAULT_STRENGTH for halfSize, never infinite size");
        assertEquals((float) DEFAULT_STRENGTH, spec.alpha(), 1e-4f,
            "+Infinity strength must fall back to DEFAULT_STRENGTH for alpha");
    }

    @Test
    void negativeInfinityStrengthFallsBackToDefault() {
        // -Inf：同样 isFinite 失败 → DEFAULT_STRENGTH（不被当成「小于 0 → clamp 0」）。
        OffscreenRelicRevealPlayer.DecalSpec spec = OffscreenRelicRevealPlayer.resolveDecalSpec(
            payload(OptionalInt.of(0x102030), Optional.of(Double.NEGATIVE_INFINITY), OptionalInt.of(100))
        );

        assertEquals(HALF_SIZE_BASE + HALF_SIZE_PER_STRENGTH * DEFAULT_STRENGTH, spec.halfSize(), EPS,
            "-Infinity strength must fall back to DEFAULT_STRENGTH (isFinite guard precedes the clamp), not 0");
        assertEquals((float) DEFAULT_STRENGTH, spec.alpha(), 1e-4f,
            "-Infinity strength must fall back to DEFAULT_STRENGTH for alpha, not the ALPHA_MIN floor");
    }

    @Test
    void durationBelowMinClampsToFloor() {
        // lifetime 越下界（含 0 / 负）：clamp 到 MAX_AGE_MIN，绝不给 0/负的寿命。
        OffscreenRelicRevealPlayer.DecalSpec specZero = OffscreenRelicRevealPlayer.resolveDecalSpec(
            payload(OptionalInt.of(0x102030), Optional.of(0.5), OptionalInt.of(0))
        );
        assertEquals(MAX_AGE_MIN, specZero.maxAge(),
            "durationTicks=0 must clamp up to MAX_AGE_MIN=60 (a decal must outlive a single frame)");

        OffscreenRelicRevealPlayer.DecalSpec specNeg = OffscreenRelicRevealPlayer.resolveDecalSpec(
            payload(OptionalInt.of(0x102030), Optional.of(0.5), OptionalInt.of(-30))
        );
        assertEquals(MAX_AGE_MIN, specNeg.maxAge(),
            "negative durationTicks must clamp up to MAX_AGE_MIN=60");

        // off-by-one just below the floor.
        OffscreenRelicRevealPlayer.DecalSpec specBelow = OffscreenRelicRevealPlayer.resolveDecalSpec(
            payload(OptionalInt.of(0x102030), Optional.of(0.5), OptionalInt.of(MAX_AGE_MIN - 1))
        );
        assertEquals(MAX_AGE_MIN, specBelow.maxAge(),
            "durationTicks=MAX_AGE_MIN-1 (59) must clamp up to the floor 60");
    }

    @Test
    void durationAboveMaxClampsToCap() {
        // lifetime 越上界：clamp 到 MAX_AGE_MAX。
        OffscreenRelicRevealPlayer.DecalSpec specHuge = OffscreenRelicRevealPlayer.resolveDecalSpec(
            payload(OptionalInt.of(0x102030), Optional.of(0.5), OptionalInt.of(100_000))
        );
        assertEquals(MAX_AGE_MAX, specHuge.maxAge(),
            "huge durationTicks must clamp down to MAX_AGE_MAX=200");

        // off-by-one just above the cap.
        OffscreenRelicRevealPlayer.DecalSpec specAbove = OffscreenRelicRevealPlayer.resolveDecalSpec(
            payload(OptionalInt.of(0x102030), Optional.of(0.5), OptionalInt.of(MAX_AGE_MAX + 1))
        );
        assertEquals(MAX_AGE_MAX, specAbove.maxAge(),
            "durationTicks=MAX_AGE_MAX+1 (201) must clamp down to the cap 200");
    }

    @Test
    void durationAtBoundsPassesThroughUnchanged() {
        // 边界 on：恰好 MIN / MAX 原样通过（off-by-one 守住「等于边界不被改」）。
        OffscreenRelicRevealPlayer.DecalSpec specMin = OffscreenRelicRevealPlayer.resolveDecalSpec(
            payload(OptionalInt.of(0x102030), Optional.of(0.5), OptionalInt.of(MAX_AGE_MIN))
        );
        assertEquals(MAX_AGE_MIN, specMin.maxAge(),
            "durationTicks exactly at MAX_AGE_MIN=60 must pass through unchanged");

        OffscreenRelicRevealPlayer.DecalSpec specMax = OffscreenRelicRevealPlayer.resolveDecalSpec(
            payload(OptionalInt.of(0x102030), Optional.of(0.5), OptionalInt.of(MAX_AGE_MAX))
        );
        assertEquals(MAX_AGE_MAX, specMax.maxAge(),
            "durationTicks exactly at MAX_AGE_MAX=200 must pass through unchanged");

        // 区间内任意值原样通过。
        OffscreenRelicRevealPlayer.DecalSpec specMid = OffscreenRelicRevealPlayer.resolveDecalSpec(
            payload(OptionalInt.of(0x102030), Optional.of(0.5), OptionalInt.of(123))
        );
        assertEquals(123, specMid.maxAge(),
            "an in-range durationTicks=123 must pass through unchanged");
    }
}
