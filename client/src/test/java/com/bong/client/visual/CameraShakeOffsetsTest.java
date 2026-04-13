package com.bong.client.visual;

import com.bong.client.state.VisualEffectState;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 覆盖 {@link CameraShakeOffsets} 的纯数学行为：
 * 非 SCREEN_SHAKE 状态或过期时返回零、振幅被 scaledIntensity 约束、同一时刻结果确定性。
 */
public class CameraShakeOffsetsTest {

    @Test
    void nullStateReturnsZero() {
        assertSame(CameraShakeOffsets.ZERO, CameraShakeOffsets.compute(null, 0L));
    }

    @Test
    void emptyStateReturnsZero() {
        assertSame(CameraShakeOffsets.ZERO, CameraShakeOffsets.compute(VisualEffectState.none(), 0L));
    }

    @Test
    void nonScreenShakeStateReturnsZero() {
        VisualEffectState bloodMoon = VisualEffectState.create("blood_moon", 1.0, 5_000L, 0L);
        assertSame(CameraShakeOffsets.ZERO, CameraShakeOffsets.compute(bloodMoon, 100L));
    }

    @Test
    void expiredStateReturnsZero() {
        VisualEffectState shake = VisualEffectState.create("screen_shake", 1.0, 1_000L, 0L);
        // 时间超过 duration
        assertSame(CameraShakeOffsets.ZERO, CameraShakeOffsets.compute(shake, 2_000L));
    }

    @Test
    void activeShakeProducesNonZeroOffsetsOverTime() {
        VisualEffectState shake = VisualEffectState.create("screen_shake", 1.0, 10_000L, 0L);
        // 刚开始时 yaw = sin(0) = 0，pitch = cos(0) * amp * ratio ≠ 0
        CameraShakeOffsets.Offsets start = CameraShakeOffsets.compute(shake, 0L);
        assertEquals(0f, start.yawDegrees(), 1e-5);
        assertNotEquals(0f, start.pitchDegrees());

        // 50ms 后 yaw 也偏离零（18Hz → 周期 ~55ms，50ms 刚好快到峰值区）
        CameraShakeOffsets.Offsets later = CameraShakeOffsets.compute(shake, 50L);
        assertFalse(later.isZero(), "50ms 后应产生非零抖动");
    }

    @Test
    void amplitudeBoundedByMaxDegreesScaled() {
        VisualEffectState shake = VisualEffectState.create("screen_shake", 0.5, 10_000L, 0L);
        // 采样一整个周期范围内多个点，确认最大幅度 ≤ MAX_DEGREES × intensity
        double maxYaw = 0.0;
        double maxPitch = 0.0;
        for (long t = 0L; t <= 100L; t++) {
            CameraShakeOffsets.Offsets offsets = CameraShakeOffsets.compute(shake, t);
            maxYaw = Math.max(maxYaw, Math.abs(offsets.yawDegrees()));
            maxPitch = Math.max(maxPitch, Math.abs(offsets.pitchDegrees()));
        }
        double intensityAtZero = shake.scaledIntensityAt(0L); // ~0.5
        double yawCap = CameraShakeOffsets.SHAKE_MAX_DEGREES * intensityAtZero;
        double pitchCap = yawCap * CameraShakeOffsets.SHAKE_PITCH_AMP_RATIO;
        // 允许 1% 浮点余量
        assertTrue(maxYaw <= yawCap * 1.01, "yaw 幅度 " + maxYaw + " 应 ≤ " + yawCap);
        assertTrue(maxPitch <= pitchCap * 1.01, "pitch 幅度 " + maxPitch + " 应 ≤ " + pitchCap);
    }

    @Test
    void computeIsDeterministic() {
        VisualEffectState shake = VisualEffectState.create("screen_shake", 1.0, 10_000L, 0L);
        CameraShakeOffsets.Offsets a = CameraShakeOffsets.compute(shake, 123L);
        CameraShakeOffsets.Offsets b = CameraShakeOffsets.compute(shake, 123L);
        assertEquals(a.yawDegrees(), b.yawDegrees(), 0f);
        assertEquals(a.pitchDegrees(), b.pitchDegrees(), 0f);
    }

    @Test
    void decayShrinksOffsetNearEndOfDuration() {
        VisualEffectState shake = VisualEffectState.create("screen_shake", 1.0, 1_000L, 0L);
        // 取同一相位点（频率整数倍能整除的时刻难算，这里比较绝对峰值随时间的下降趋势）
        double earlyPeak = 0.0;
        double latePeak = 0.0;
        // 前 200ms 内的峰值
        for (long t = 0L; t <= 200L; t++) {
            double v = Math.abs(CameraShakeOffsets.compute(shake, t).yawDegrees());
            earlyPeak = Math.max(earlyPeak, v);
        }
        // 800..1000ms 内的峰值
        for (long t = 800L; t <= 1000L; t++) {
            double v = Math.abs(CameraShakeOffsets.compute(shake, t).yawDegrees());
            latePeak = Math.max(latePeak, v);
        }
        assertTrue(latePeak < earlyPeak,
            "接近结尾的峰值 " + latePeak + " 应小于开头峰值 " + earlyPeak);
    }

    @Test
    void offsetsRecordIsZeroHelperWorks() {
        assertTrue(CameraShakeOffsets.ZERO.isZero());
        assertFalse(new CameraShakeOffsets.Offsets(0.1f, 0f).isZero());
        assertFalse(new CameraShakeOffsets.Offsets(0f, 0.1f).isZero());
    }

    @Test
    void pressureJitterProducesLowerAmplitudeThanScreenShake() {
        VisualEffectState shake = VisualEffectState.create("screen_shake", 1.0, 10_000L, 0L);
        VisualEffectState jitter = VisualEffectState.create("pressure_jitter", 1.0, 10_000L, 0L);

        double shakePeak = 0.0;
        double jitterPeak = 0.0;
        // 采样足够长（>=1s）以覆盖低频（6Hz）的至少一个完整周期
        for (long t = 0L; t <= 1_000L; t += 5L) {
            shakePeak = Math.max(shakePeak, Math.abs(CameraShakeOffsets.compute(shake, t).yawDegrees()));
            jitterPeak = Math.max(jitterPeak, Math.abs(CameraShakeOffsets.compute(jitter, t).yawDegrees()));
        }
        assertTrue(jitterPeak < shakePeak,
            "灵压 jitter 峰值 " + jitterPeak + " 应 < shake 峰值 " + shakePeak);
        // jitter 峰值上限应在 JITTER_MAX_DEGREES × 1.01 以内
        assertTrue(jitterPeak <= CameraShakeOffsets.JITTER_MAX_DEGREES * 1.01,
            "jitter 峰值 " + jitterPeak + " 应 ≤ " + CameraShakeOffsets.JITTER_MAX_DEGREES);
    }

    @Test
    void pressureJitterDecaysLikeShake() {
        VisualEffectState jitter = VisualEffectState.create("pressure_jitter", 1.0, 1_000L, 0L);
        double earlyPeak = 0.0;
        double latePeak = 0.0;
        for (long t = 0L; t <= 200L; t += 5L) {
            earlyPeak = Math.max(earlyPeak, Math.abs(CameraShakeOffsets.compute(jitter, t).yawDegrees()));
        }
        for (long t = 800L; t <= 1000L; t += 5L) {
            latePeak = Math.max(latePeak, Math.abs(CameraShakeOffsets.compute(jitter, t).yawDegrees()));
        }
        assertTrue(latePeak < earlyPeak, "jitter 末段峰值 " + latePeak + " 应 < 开头 " + earlyPeak);
    }

    @Test
    void nonShakeNonJitterEffectsReturnZero() {
        // 既不是 SCREEN_SHAKE 也不是 PRESSURE_JITTER 时，compute 应该直通零值
        assertSame(CameraShakeOffsets.ZERO, CameraShakeOffsets.compute(
            VisualEffectState.create("blood_moon", 1.0, 5_000L, 0L), 100L));
        assertSame(CameraShakeOffsets.ZERO, CameraShakeOffsets.compute(
            VisualEffectState.create("fov_zoom_in", 1.0, 5_000L, 0L), 100L));
        assertSame(CameraShakeOffsets.ZERO, CameraShakeOffsets.compute(
            VisualEffectState.create("tribulation_look_up", 1.0, 5_000L, 0L), 100L));
    }
}
