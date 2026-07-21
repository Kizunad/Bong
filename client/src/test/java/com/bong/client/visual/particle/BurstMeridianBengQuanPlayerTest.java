package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import org.junit.jupiter.api.Test;

import java.util.Optional;
import java.util.OptionalInt;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * headless 锁住 {@link BurstMeridianBengQuanPlayer#resolveBurstSpec} 的全部
 * 「payload -> 归一化渲染标量」分支。该纯函数把旧内联实现里手搓的 rgb 拆解 / 方向归一 /
 * count clamp 收编进 {@link GameplayVfxUtil}，并补上两处旧实现缺失的健壮性守卫——
 * {@code strength} 的 {@link Double#isFinite} 前置守卫（NaN/±Inf → 默认，绝不传 NaN 进渲染标量）、
 * {@code maxAge} 的 clamp[1,600]（旧实现不 clamp）。这些算术全部可在无 {@code MinecraftClient}
 * 的 JVM 里断言；{@code play()} 里的逐粒子几何 + particleManager 提交仍需 live client，留在
 * 人工 runClient checklist，本类不碰。
 */
class BurstMeridianBengQuanPlayerTest {

    // 与被测常量手动对齐（被测里是 private；这里复刻，改任一端都会撞红 —— 有意的 pin）。
    private static final int FALLBACK_RGB = 0xC58B3F;
    private static final double DEFAULT_STRENGTH = 0.9;
    private static final int DEFAULT_COUNT = 8;
    private static final int MIN_COUNT = 1;
    private static final int MAX_COUNT = 24;
    private static final int DEFAULT_DURATION = 8;
    private static final int MIN_DURATION = 1;
    private static final int MAX_DURATION = 600;

    private static final double EPS = 1e-9;

    private static VfxEventPayload.SpawnParticle payload(
        Optional<double[]> direction,
        OptionalInt colorRgb,
        Optional<Double> strength,
        OptionalInt count,
        OptionalInt durationTicks
    ) {
        return new VfxEventPayload.SpawnParticle(
            BurstMeridianBengQuanPlayer.EVENT_ID,
            new double[] { 10.0, 65.0, -4.0 },
            direction,
            colorRgb,
            strength,
            count,
            durationTicks
        );
    }

    /** 只覆盖 color，其余给合法默认（in-range，不触发任何 clamp）。 */
    private static VfxEventPayload.SpawnParticle withColor(OptionalInt colorRgb) {
        return payload(Optional.empty(), colorRgb, Optional.of(0.5), OptionalInt.of(10), OptionalInt.of(8));
    }

    /** 只覆盖 strength，其余给合法默认。 */
    private static VfxEventPayload.SpawnParticle withStrength(Optional<Double> strength) {
        return payload(Optional.empty(), OptionalInt.of(0x336699), strength, OptionalInt.of(10), OptionalInt.of(8));
    }

    /** 只覆盖 count，其余给合法默认。 */
    private static VfxEventPayload.SpawnParticle withCount(OptionalInt count) {
        return payload(Optional.empty(), OptionalInt.of(0x336699), Optional.of(0.5), count, OptionalInt.of(8));
    }

    /** 只覆盖 duration，其余给合法默认。 */
    private static VfxEventPayload.SpawnParticle withDuration(OptionalInt durationTicks) {
        return payload(Optional.empty(), OptionalInt.of(0x336699), Optional.of(0.5), OptionalInt.of(10), durationTicks);
    }

    /** 只覆盖 direction，其余给合法默认。 */
    private static VfxEventPayload.SpawnParticle withDirection(Optional<double[]> direction) {
        return payload(direction, OptionalInt.of(0x336699), Optional.of(0.5), OptionalInt.of(10), OptionalInt.of(8));
    }

    private static float channel(int rgb, int shift) {
        return ((rgb >> shift) & 0xFF) / 255f;
    }

    // ---- happy path：所有字段合法时整体逐值解析 ----

    @Test
    void happyPathResolvesEveryFieldFromPayload() {
        BurstMeridianBengQuanPlayer.BurstSpec spec = BurstMeridianBengQuanPlayer.resolveBurstSpec(
            payload(Optional.of(new double[] { 0.0, 0.0, 2.0 }), OptionalInt.of(0x336699),
                Optional.of(0.5), OptionalInt.of(12), OptionalInt.of(15))
        );

        assertEquals(channel(0x336699, 16), spec.red(), 1e-6f, "red 必须来自 payload.color 高字节");
        assertEquals(channel(0x336699, 8), spec.green(), 1e-6f, "green 必须来自 payload.color 中字节");
        assertEquals(channel(0x336699, 0), spec.blue(), 1e-6f, "blue 必须来自 payload.color 低字节");
        assertEquals(0.0, spec.dirX(), EPS, "方向 {0,0,2} 归一后 x 分量应为 0");
        assertEquals(0.0, spec.dirY(), EPS, "方向 {0,0,2} 归一后 y 分量应为 0");
        assertEquals(1.0, spec.dirZ(), EPS, "方向 {0,0,2} 归一后 z 分量应为 1（模长 2 归一）");
        assertEquals(0.5, spec.strength(), EPS, "in-range strength=0.5 原样通过");
        assertEquals(12, spec.count(), "in-range count=12 原样通过");
        assertEquals(15, spec.maxAge(), "in-range durationTicks=15 原样通过");
    }

    // ---- RGB ----

    @Test
    void colorChannelsSplitFromPayload() {
        BurstMeridianBengQuanPlayer.BurstSpec spec =
            BurstMeridianBengQuanPlayer.resolveBurstSpec(withColor(OptionalInt.of(0x102030)));
        assertEquals(channel(0x102030, 16), spec.red(), 1e-6f, "red = 0x10/255");
        assertEquals(channel(0x102030, 8), spec.green(), 1e-6f, "green = 0x20/255");
        assertEquals(channel(0x102030, 0), spec.blue(), 1e-6f, "blue = 0x30/255");
    }

    @Test
    void missingColorFallsBackToBronze() {
        BurstMeridianBengQuanPlayer.BurstSpec spec =
            BurstMeridianBengQuanPlayer.resolveBurstSpec(withColor(OptionalInt.empty()));
        assertEquals(channel(FALLBACK_RGB, 16), spec.red(), 1e-6f, "缺 color 必须回落古铜 #C58B3F 红通道");
        assertEquals(channel(FALLBACK_RGB, 8), spec.green(), 1e-6f, "缺 color 回落古铜绿通道");
        assertEquals(channel(FALLBACK_RGB, 0), spec.blue(), 1e-6f, "缺 color 回落古铜蓝通道");
    }

    // ---- direction ----

    @Test
    void directionNormalizedToUnitLength() {
        // {3,0,4} 模长 5 → {0.6, 0, 0.8}
        BurstMeridianBengQuanPlayer.BurstSpec spec = BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withDirection(Optional.of(new double[] { 3.0, 0.0, 4.0 })));
        assertEquals(0.6, spec.dirX(), EPS, "{3,0,4} 归一 x=0.6");
        assertEquals(0.0, spec.dirY(), EPS, "{3,0,4} 归一 y=0");
        assertEquals(0.8, spec.dirZ(), EPS, "{3,0,4} 归一 z=0.8");
    }

    @Test
    void missingDirectionFallsBackToPositiveX() {
        BurstMeridianBengQuanPlayer.BurstSpec spec =
            BurstMeridianBengQuanPlayer.resolveBurstSpec(withDirection(Optional.empty()));
        assertEquals(1.0, spec.dirX(), EPS, "缺方向回落 {1,0,0}");
        assertEquals(0.0, spec.dirY(), EPS, "缺方向回落 y=0");
        assertEquals(0.0, spec.dirZ(), EPS, "缺方向回落 z=0");
    }

    @Test
    void zeroVectorDirectionFallsBackToPositiveX() {
        BurstMeridianBengQuanPlayer.BurstSpec spec = BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withDirection(Optional.of(new double[] { 0.0, 0.0, 0.0 })));
        assertEquals(1.0, spec.dirX(), EPS, "零向量（模长<=1e-6）必须回落 {1,0,0}，绝不除零产 NaN");
        assertEquals(0.0, spec.dirY(), EPS, "零向量回落 y=0");
        assertEquals(0.0, spec.dirZ(), EPS, "零向量回落 z=0");
    }

    // 注：短方向数组（length<3）无法测——SpawnParticle record 紧凑构造器强制 direction 若存在必长度 3
    // （VfxEventPayload.java:145-150），length<3 在构造期即抛，是 resolveBurstSpec 的不可达输入。

    // ---- strength：happy / clamp / isFinite 守卫 ----

    @Test
    void inRangeStrengthPassesThrough() {
        assertEquals(0.5, BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withStrength(Optional.of(0.5))).strength(), EPS, "in-range strength=0.5 原样");
    }

    @Test
    void missingStrengthUsesDefault() {
        assertEquals(DEFAULT_STRENGTH, BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withStrength(Optional.empty())).strength(), EPS, "缺 strength 用默认 0.9");
    }

    @Test
    void strengthBoundsPassThrough() {
        assertEquals(0.0, BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withStrength(Optional.of(0.0))).strength(), EPS, "下界 0.0 原样通过");
        assertEquals(1.0, BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withStrength(Optional.of(1.0))).strength(), EPS, "上界 1.0 原样通过");
    }

    @Test
    void negativeStrengthClampsToZero() {
        assertEquals(0.0, BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withStrength(Optional.of(-0.3))).strength(), EPS, "负 strength clamp 到 0，绝不产负速度/线宽");
    }

    @Test
    void oversizedStrengthClampsToOne() {
        assertEquals(1.0, BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withStrength(Optional.of(1.7))).strength(), EPS, "超 1 的 strength clamp 到 1");
    }

    @Test
    void nanStrengthFallsBackToDefaultNotPropagated() {
        double s = BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withStrength(Optional.of(Double.NaN))).strength();
        assertTrue(Double.isFinite(s), "NaN strength 必须被 isFinite 守卫拦下，结果保持有限");
        assertEquals(DEFAULT_STRENGTH, s, EPS, "NaN strength 回落 0.9（旧实现会把 NaN 传进 alpha/速度/线宽）");
    }

    @Test
    void positiveInfinityStrengthFallsBackToDefault() {
        assertEquals(DEFAULT_STRENGTH, BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withStrength(Optional.of(Double.POSITIVE_INFINITY))).strength(), EPS,
            "+Inf strength 回落 0.9，绝不算出无穷速度");
    }

    @Test
    void negativeInfinityStrengthFallsBackToDefaultNotClampedToZero() {
        assertEquals(DEFAULT_STRENGTH, BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withStrength(Optional.of(Double.NEGATIVE_INFINITY))).strength(), EPS,
            "-Inf 必须走 isFinite 守卫回落 0.9，而不是被当成 <0 clamp 到 0");
    }

    // ---- count：clamp[1,24] ----

    @Test
    void inRangeCountPassesThrough() {
        assertEquals(12, BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withCount(OptionalInt.of(12))).count(), "in-range count=12 原样");
    }

    @Test
    void missingCountUsesDefault() {
        assertEquals(DEFAULT_COUNT, BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withCount(OptionalInt.empty())).count(), "缺 count 用默认 8");
    }

    @Test
    void countBoundsPassThrough() {
        assertEquals(MIN_COUNT, BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withCount(OptionalInt.of(MIN_COUNT))).count(), "下界 1 原样");
        assertEquals(MAX_COUNT, BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withCount(OptionalInt.of(MAX_COUNT))).count(), "上界 24 原样");
    }

    @Test
    void countBelowMinClampsUp() {
        assertEquals(MIN_COUNT, BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withCount(OptionalInt.of(0))).count(), "count=0 clamp 到 1（至少发一条）");
        assertEquals(MIN_COUNT, BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withCount(OptionalInt.of(-5))).count(), "负 count clamp 到 1");
    }

    @Test
    void countAboveMaxClampsDown() {
        assertEquals(MAX_COUNT, BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withCount(OptionalInt.of(30))).count(), "count=30 clamp 到 24（防粒子风暴）");
    }

    // ---- maxAge（duration）：clamp[1,600]，旧实现完全不 clamp ----

    @Test
    void inRangeDurationPassesThrough() {
        assertEquals(15, BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withDuration(OptionalInt.of(15))).maxAge(), "in-range durationTicks=15 原样");
    }

    @Test
    void missingDurationUsesDefault() {
        assertEquals(DEFAULT_DURATION, BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withDuration(OptionalInt.empty())).maxAge(), "缺 durationTicks 用默认 8");
    }

    @Test
    void durationBoundsPassThrough() {
        assertEquals(MIN_DURATION, BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withDuration(OptionalInt.of(MIN_DURATION))).maxAge(), "下界 1 原样");
        assertEquals(MAX_DURATION, BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withDuration(OptionalInt.of(MAX_DURATION))).maxAge(), "上界 600 原样");
    }

    @Test
    void durationBelowMinClampsUp() {
        // 旧实现直接把 0/负数塞进 setMaxAgePublic —— 这正是本次加固修掉的洞。
        assertEquals(MIN_DURATION, BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withDuration(OptionalInt.of(0))).maxAge(), "durationTicks=0 必须 clamp 到 1（粒子至少活一帧）");
        assertEquals(MIN_DURATION, BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withDuration(OptionalInt.of(-30))).maxAge(), "负 durationTicks 必须 clamp 到 1");
    }

    @Test
    void durationAboveMaxClampsDown() {
        assertEquals(MAX_DURATION, BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withDuration(OptionalInt.of(100_000))).maxAge(), "超大 durationTicks clamp 到 600，绝不留千帧幽灵粒子");
        assertEquals(MAX_DURATION, BurstMeridianBengQuanPlayer.resolveBurstSpec(
            withDuration(OptionalInt.of(MAX_DURATION + 1))).maxAge(), "601（越上界 off-by-one）clamp 到 600");
    }
}
