package com.bong.client.visual;

import com.bong.client.state.VisualEffectState;

/**
 * 从 {@link VisualEffectState} 计算要叠加到 {@code GameRenderer.getFov()} 返回值上的 FOV 度数偏移。
 *
 * 仅被 {@code MixinGameRenderer} 与其单测使用：纯数学、无副作用、确定性。
 *
 * <ul>
 *   <li>{@code FOV_ZOOM_IN}：返回负值，使视野收紧（运功专注感）。scaledIntensityAt 衰减 → 自然回到默认 FOV。</li>
 *   <li>{@code FOV_STRETCH}：返回正值，使视野外推（破境爆发感）。短 duration → 快速回弹。</li>
 *   <li>其他效果 / 空 state：返回 0，调用方可直通。</li>
 * </ul>
 */
public final class CameraFovOffset {
    /** 运功 FOV 收缩峰值（度）。默认 FOV 70 → 满强度下收到 ~55，足够"专注"而不会让界面过窄眩晕。 */
    static final double MAX_ZOOM_DEGREES = 15.0;
    /** 破境 FOV 拉伸峰值（度）。默认 FOV 70 → 瞬时推到 ~90，制造广角爆发，短 duration 快速回落。 */
    static final double MAX_STRETCH_DEGREES = 20.0;

    private CameraFovOffset() {
    }

    public static double compute(VisualEffectState state, long nowMillis) {
        if (state == null || state.isEmpty()) {
            return 0.0;
        }
        double scaled = state.scaledIntensityAt(nowMillis);
        if (scaled <= 0.0) {
            return 0.0;
        }
        return switch (state.effectType()) {
            case FOV_ZOOM_IN -> -MAX_ZOOM_DEGREES * scaled;
            case FOV_STRETCH -> MAX_STRETCH_DEGREES * scaled;
            default -> 0.0;
        };
    }
}
