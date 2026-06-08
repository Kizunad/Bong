package com.bong.client.era;

/**
 * plan-era-state-v1 P3 — 时代天象 S2C payload 值对象。
 *
 * <p>从 {@code bong:era_ambiance} JSON 解析后存储的不可变记录。
 * 不做渲染，只携带数据供渲染层消费。
 *
 * <h3>字段说明</h3>
 * <ul>
 *   <li>{@link #eraType()}：时代 wire 字符串（"calamity"/"change"/"deduction"/"unknown"）</li>
 *   <li>{@link #skyTintRgb()}：天空色调 RGB int；{@code 0x000000} = 不修改（reset/baseline）</li>
 *   <li>{@link #fogDensityDelta()}：雾密度增量（-1.0..1.0）</li>
 *   <li>{@link #ambientSoundId()}：ambient sound id（空字符串 = 无音效/stop）</li>
 *   <li>{@link #transitionTicks()}：渐变过渡时长（tick），通常为 1200（60s @20TPS）</li>
 * </ul>
 */
public record EraAmbiancePayload(
    String eraType,
    int skyTintRgb,
    double fogDensityDelta,
    String ambientSoundId,
    int transitionTicks
) {
    /**
     * 是否为 reset 态（未知/无时代）：era_type == "unknown"。
     * reset 时 client 应清零之前叠加的天象效果（渐变归零）。
     */
    public boolean isReset() {
        return "unknown".equals(eraType);
    }

    /**
     * 天空色调是否有效（非黑色 baseline）。
     * {@code skyTintRgb == 0x000000} 时 client 不修改天空色调。
     */
    public boolean hasSkyTint() {
        return (skyTintRgb & 0x00FFFFFF) != 0;
    }

    /**
     * 是否有 ambient sound id。
     */
    public boolean hasAmbientSound() {
        return ambientSoundId != null && !ambientSoundId.isBlank();
    }
}
