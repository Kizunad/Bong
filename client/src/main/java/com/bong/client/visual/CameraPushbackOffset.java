package com.bong.client.visual;

import com.bong.client.state.VisualEffectState;

/**
 * 为 HIT_PUSHBACK 计算相机沿 facing 反方向的后退距离（单位：格 / blocks）。
 *
 * <p>随 {@link VisualEffectState#scaledIntensityAt} 线性衰减：受击瞬间相机推到最远，
 * 然后在 {@code durationMillis} 内线性回到原位。配合短 duration（350ms）形成"被撞后猛地后仰再回头"的视觉节奏。
 *
 * <p>调用方（{@code MixinCamera}）用 {@code moveBy(-distance, 0, 0)} 让相机沿 camera-local 的 -x
 * 方向移动——这正是 MC 第三人称拉远用的方向，相当于"远离面朝方向"。第一/第三人称通用。
 */
public final class CameraPushbackOffset {
    /** 峰值后退距离（格）。0.5 格 ≈ 半格身位，足够有"被撞退"感又不会让视野剧烈错位。 */
    static final double MAX_PUSHBACK_BLOCKS = 0.5;

    private CameraPushbackOffset() {
    }

    public static double computeBackwardDistance(VisualEffectState state, long nowMillis) {
        if (state == null || state.isEmpty()
            || state.effectType() != VisualEffectState.EffectType.HIT_PUSHBACK) {
            return 0.0;
        }
        double scaled = state.scaledIntensityAt(nowMillis);
        if (scaled <= 0.0) {
            return 0.0;
        }
        return MAX_PUSHBACK_BLOCKS * scaled;
    }
}
