package com.bong.client.visual;

import com.bong.client.state.VisualEffectState;

/**
 * 为 TRIBULATION_LOOK_UP 计算单调 pitch 偏移，配合 {@code MixinCamera} 让相机自动抬头仰望天空。
 *
 * <p>走钟形曲线 {@code sin(π · progress)}：
 * <ul>
 *   <li>{@code progress=0}（刚触发）→ 偏移 0°，不会突然抬头</li>
 *   <li>{@code progress=0.5}（正中间）→ 偏移达到 {@code -MAX_TILT_DEGREES × intensity}</li>
 *   <li>{@code progress=1}（结束）→ 偏移回到 0°，不会突然放下</li>
 * </ul>
 *
 * <p>注意 MC 约定：pitch 负值=看天空，正值=看地面。本工具返回负值，单位为度。
 *
 * <p>{@code MixinCamera} 已有 shake 偏移逻辑，本工具返回的偏移**叠加**到 shake 上：
 * 天劫降临 = 抬头 + 抖动，两种效果自然并存。
 */
public final class CameraTiltOffset {
    /** 最大仰角（度）。25° 足够看到天空，又不会让玩家完全失去地面参考。 */
    static final double MAX_TILT_DEGREES = 25.0;

    private CameraTiltOffset() {
    }

    public static float computePitchDegrees(VisualEffectState state, long nowMillis) {
        if (state == null || state.isEmpty()
            || state.effectType() != VisualEffectState.EffectType.TRIBULATION_LOOK_UP) {
            return 0f;
        }
        long duration = state.durationMillis();
        if (duration <= 0L) {
            return 0f;
        }
        long elapsed = Math.max(0L, nowMillis - Math.max(0L, state.startedAtMillis()));
        if (elapsed >= duration) {
            return 0f;
        }
        double progress = (double) elapsed / (double) duration;
        // sin(π·progress) 给出 0 → 1 → 0 的钟形，自动处理 ramp-up 和 ramp-down
        double bell = Math.sin(progress * Math.PI);
        double pitch = -MAX_TILT_DEGREES * state.intensity() * bell;
        return (float) pitch;
    }
}
