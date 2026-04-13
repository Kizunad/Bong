package com.bong.client.visual;

import com.bong.client.state.VisualEffectState;

/**
 * 基于相机抖动类 VisualEffectState 计算 yaw/pitch 偏移。
 *
 * <p>当前支持两种档位，走同一套双频正弦数学，仅 amp/freq 不同：
 * <ul>
 *   <li>{@code SCREEN_SHAKE}：1.5° 高幅 18Hz yaw / 14.5Hz pitch，"天道警示"级烈度</li>
 *   <li>{@code PRESSURE_JITTER}：0.3° 低幅 6Hz / 5Hz，"灵压心跳"般的压迫感</li>
 * </ul>
 *
 * <p>频率偏移两个不同值让晃动感觉凌乱不是周期循环；振幅 = {@code maxDegrees × scaledIntensityAt}。
 * 非抖动类 state（overlays/FOV/tilt）返回零，等价于直通。
 */
public final class CameraShakeOffsets {
    /** SCREEN_SHAKE 峰值偏移（度），1.5° 约 3 格宽度感。 */
    static final double SHAKE_MAX_DEGREES = 1.5;
    /** SCREEN_SHAKE Yaw 频率（Hz）。 */
    static final double SHAKE_YAW_FREQ_HZ = 18.0;
    /** SCREEN_SHAKE Pitch 频率（Hz），与 yaw 错开避免呈周期性打圈。 */
    static final double SHAKE_PITCH_FREQ_HZ = 14.5;
    /** SCREEN_SHAKE Pitch 幅度相对 yaw 的系数。 */
    static final double SHAKE_PITCH_AMP_RATIO = 0.7;

    /** PRESSURE_JITTER 峰值偏移（度），低幅营造心跳压迫感而非眩晕震动。 */
    static final double JITTER_MAX_DEGREES = 0.3;
    /** PRESSURE_JITTER Yaw 频率（Hz），低频接近"胸腔起伏"节奏。 */
    static final double JITTER_YAW_FREQ_HZ = 6.0;
    /** PRESSURE_JITTER Pitch 频率（Hz）。 */
    static final double JITTER_PITCH_FREQ_HZ = 5.0;
    /** PRESSURE_JITTER Pitch 幅度系数，比 shake 更接近 1.0 让上下起伏更明显。 */
    static final double JITTER_PITCH_AMP_RATIO = 0.9;

    public static final Offsets ZERO = new Offsets(0f, 0f);

    private CameraShakeOffsets() {
    }

    public static Offsets compute(VisualEffectState state, long nowMillis) {
        if (state == null || state.isEmpty()) {
            return ZERO;
        }
        Params params = paramsFor(state.effectType());
        if (params == null) {
            return ZERO;
        }
        double scaled = state.scaledIntensityAt(nowMillis);
        if (scaled <= 0.0) {
            return ZERO;
        }

        long elapsedMillis = Math.max(0L, nowMillis - state.startedAtMillis());
        double seconds = elapsedMillis / 1000.0;
        double amplitudeDegrees = params.maxDegrees() * scaled;

        float yawOffset = (float) (Math.sin(seconds * 2.0 * Math.PI * params.yawFreqHz()) * amplitudeDegrees);
        float pitchOffset = (float) (Math.cos(seconds * 2.0 * Math.PI * params.pitchFreqHz()) * amplitudeDegrees * params.pitchAmpRatio());
        return new Offsets(yawOffset, pitchOffset);
    }

    private static Params paramsFor(VisualEffectState.EffectType type) {
        return switch (type) {
            case SCREEN_SHAKE -> new Params(SHAKE_MAX_DEGREES, SHAKE_YAW_FREQ_HZ, SHAKE_PITCH_FREQ_HZ, SHAKE_PITCH_AMP_RATIO);
            case PRESSURE_JITTER -> new Params(JITTER_MAX_DEGREES, JITTER_YAW_FREQ_HZ, JITTER_PITCH_FREQ_HZ, JITTER_PITCH_AMP_RATIO);
            default -> null;
        };
    }

    private record Params(double maxDegrees, double yawFreqHz, double pitchFreqHz, double pitchAmpRatio) {
    }

    public record Offsets(float yawDegrees, float pitchDegrees) {
        public boolean isZero() {
            return yawDegrees == 0f && pitchDegrees == 0f;
        }
    }
}
