package com.bong.client.inventory.model;

import java.util.List;

/**
 * 模拟经脉状态 — 一个使用爆脉流后受伤的炼气三层修士。
 */
public final class MockMeridianData {
    private MockMeridianData() {}

    public static MeridianBody create() {
        double baseCap = 30.0; // 炼气期基础流量

        return MeridianBody.builder()
            .realm("炼气三层")
            // 正常的脉
            .channel(ChannelState.healthy(MeridianChannel.REN_MAI, baseCap))
            .channel(ChannelState.healthy(MeridianChannel.DU_MAI, baseCap))
            .channel(ChannelState.healthy(MeridianChannel.SPIRIT, baseCap * 0.8))
            .channel(ChannelState.healthy(MeridianChannel.LEG_YIN, baseCap))
            .channel(ChannelState.healthy(MeridianChannel.LEG_YANG, baseCap))
            .channel(ChannelState.healthy(MeridianChannel.LUNG, baseCap * 0.9))
            .channel(ChannelState.healthy(MeridianChannel.SPLEEN, baseCap * 0.7))
            // 爆脉流后遗症：右臂撕裂
            .channel(new ChannelState(MeridianChannel.ARM_YANG, baseCap, 12.0,
                ChannelState.DamageLevel.TORN, 0.0, 0.15, false))
            // 左臂有污染（被人打过）
            .channel(new ChannelState(MeridianChannel.ARM_YIN, baseCap, 22.0,
                ChannelState.DamageLevel.MICRO_TEAR, 0.25, 0.0, false))
            // 心脉微裂 — 差点走火入魔
            .channel(new ChannelState(MeridianChannel.HEART, baseCap * 1.2, 28.0,
                ChannelState.DamageLevel.MICRO_TEAR, 0.08, 0.3, false))
            // 肾脉正在恢复
            .channel(new ChannelState(MeridianChannel.KIDNEY, baseCap * 0.9, 20.0,
                ChannelState.DamageLevel.MICRO_TEAR, 0.0, 0.65, false))
            // 肝脉正常但流量偏低
            .channel(new ChannelState(MeridianChannel.LIVER, baseCap * 0.8, 15.0,
                ChannelState.DamageLevel.INTACT, 0.0, 0.0, false))
            // 丹田
            .dantian(new MeridianBody.DantianState(MeridianBody.DantianTier.UPPER, 18.0, 24.0, false))
            .dantian(new MeridianBody.DantianState(MeridianBody.DantianTier.MIDDLE, 52.0, 80.0, false))
            .dantian(new MeridianBody.DantianState(MeridianBody.DantianTier.LOWER, 35.0, 50.0, false))
            // 状态效果
            .activeEffects(List.of(
                new MeridianBody.StatusEffect("meridian_overload", "经脉过载余波",
                    "爆脉流后遗症，右臂经脉持续微痛", 0xFFCC6644, 0.4),
                new MeridianBody.StatusEffect("contamination_slow", "外源侵蚀",
                    "左臂残余外源真元未清除", 0xFF9944CC, 0.25)
            ))
            .build();
    }
}
