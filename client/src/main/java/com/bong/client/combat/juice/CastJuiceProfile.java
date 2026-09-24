package com.bong.client.combat.juice;

/**
 * plan-fpv-cast-av-v1 P3 —— 单招施法 release juice 参数（按招 pin，见 §P3 参数表）。
 *
 * <p>只给**重型招**配 juice（沉浸式极简：其余招默认零，注册表里不登记即无 juice）。
 * shake 复用 {@link CameraShakeController} 既有通道；FOV 走 {@link CastFovController} 加法脉冲。
 *
 * @param skillId          技能 id（点号形态，如 {@code sword_path.heaven_gate}），对齐服务端
 *                         TECHNIQUE_DEFINITIONS / client SkillBarEntry.id()
 * @param shakeIntensity   相机抖动强度（0 = 无 shake）；映射 §P3 表「强/中/弱」
 * @param shakeDurationTicks 抖动时长（tick）
 * @param fovPeakDegrees   FOV 脉冲峰值加法偏移（度，0 = 无脉冲）；正 = 视野扩张回弹
 * @param fovDurationTicks FOV 脉冲时长（tick），脉冲 0→peak→0 半弧回弹
 */
public record CastJuiceProfile(
    String skillId,
    float shakeIntensity,
    int shakeDurationTicks,
    float fovPeakDegrees,
    int fovDurationTicks
) {
    public boolean hasShake() {
        return shakeIntensity > 0f && shakeDurationTicks > 0;
    }

    public boolean hasFovPulse() {
        return fovPeakDegrees != 0f && fovDurationTicks > 0;
    }
}
