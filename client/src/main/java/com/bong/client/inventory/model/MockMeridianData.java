package com.bong.client.inventory.model;

import java.util.EnumMap;
import java.util.List;

/**
 * 模拟经脉状态 — 一个使用爆脉流后受伤的引气修士。
 * 覆盖 12 正经 + 8 奇经，呈现各种损伤/污染/恢复状态。
 */
public final class MockMeridianData {
    private MockMeridianData() {}

    public static MeridianBody create() {
        double baseCap = 30.0; // 引气境基础流量
        double oddCap = baseCap * 1.4; // 奇经容量更大

        MeridianBody.Builder b = MeridianBody.builder().realm("引气");

        // ===== 12 正经 =====
        // 手三阴 — 左臂主辅助，整体偏弱
        b.channel(ChannelState.healthy(MeridianChannel.LU, baseCap * 0.9));
        // 心经微裂 — 差点走火入魔
        b.channel(new ChannelState(MeridianChannel.HT, baseCap * 1.2, 28.0,
            ChannelState.DamageLevel.MICRO_TEAR, 0.08, 0.30, false));
        b.channel(ChannelState.healthy(MeridianChannel.PC, baseCap));

        // 手三阳 — 右臂爆脉流后遗症
        b.channel(new ChannelState(MeridianChannel.LI, baseCap, 22.0,
            ChannelState.DamageLevel.MICRO_TEAR, 0.25, 0.0, false));
        // 小肠经撕裂
        b.channel(new ChannelState(MeridianChannel.SI, baseCap, 12.0,
            ChannelState.DamageLevel.TORN, 0.0, 0.15, false));
        b.channel(new ChannelState(MeridianChannel.TE, baseCap * 0.95, 18.0,
            ChannelState.DamageLevel.MICRO_TEAR, 0.10, 0.20, false));

        // 足三阴 — 左腿正常
        b.channel(ChannelState.healthy(MeridianChannel.SP, baseCap * 0.85));
        // 肾经恢复中
        b.channel(new ChannelState(MeridianChannel.KI, baseCap * 0.9, 20.0,
            ChannelState.DamageLevel.MICRO_TEAR, 0.0, 0.65, false));
        b.channel(new ChannelState(MeridianChannel.LR, baseCap * 0.8, 15.0,
            ChannelState.DamageLevel.INTACT, 0.0, 0.0, false));

        // 足三阳 — 右腿正常
        b.channel(ChannelState.healthy(MeridianChannel.ST, baseCap));
        b.channel(ChannelState.healthy(MeridianChannel.BL, baseCap));
        b.channel(ChannelState.healthy(MeridianChannel.GB, baseCap * 0.95));

        // ===== 8 奇经 =====
        b.channel(ChannelState.healthy(MeridianChannel.REN, oddCap));
        b.channel(ChannelState.healthy(MeridianChannel.DU, oddCap));
        // 冲脉储满 — 真元缓存饱和
        b.channel(new ChannelState(MeridianChannel.CHONG, oddCap * 1.2, oddCap * 1.15,
            ChannelState.DamageLevel.INTACT, 0.0, 0.0, false));
        // 带脉受损 — 腰部瘀伤
        b.channel(new ChannelState(MeridianChannel.DAI, oddCap * 0.7, 14.0,
            ChannelState.DamageLevel.MICRO_TEAR, 0.05, 0.10, false));
        b.channel(ChannelState.healthy(MeridianChannel.YIN_WEI, oddCap * 0.8));
        b.channel(ChannelState.healthy(MeridianChannel.YANG_WEI, oddCap * 0.8));
        // 阴跷未通
        b.channel(new ChannelState(MeridianChannel.YIN_QIAO, oddCap * 0.6, 0.0,
            ChannelState.DamageLevel.INTACT, 0.0, 0.0, true));
        b.channel(ChannelState.healthy(MeridianChannel.YANG_QIAO, oddCap * 0.6));

        // ===== 经脉上已用物品 =====
        b.appliedItem(MeridianChannel.SI,
            InventoryItem.create("ningmai_powder", "凝脉散", 1, 1, 0.3, "uncommon", "外敷经脉，缓解走火入魔"));
        b.appliedItem(MeridianChannel.HT,
            InventoryItem.create("guyuan_pill", "固元丹", 1, 1, 0.2, "rare", "温补真元，服后可加速恢复灵力"));
        b.appliedItem(MeridianChannel.KI,
            InventoryItem.create("spirit_grass", "灵草", 1, 1, 0.5, "common", "低阶灵草，可入药炼丹"));

        // ===== 裂痕 / 污染总量（用于裂痕可视化 + 头部污染标签 QA）=====
        var cracks = new EnumMap<MeridianChannel, Integer>(MeridianChannel.class);
        cracks.put(MeridianChannel.HT, 2);
        cracks.put(MeridianChannel.SI, 4);
        cracks.put(MeridianChannel.DAI, 1);
        b.cracksCount(cracks);
        b.contaminationTotal(8.4);

        // ===== 状态效果 =====
        b.activeEffects(List.of(
            new MeridianBody.StatusEffect("meridian_overload", "经脉过载余波",
                "爆脉流后遗症，右臂经脉持续微痛", 0xFFCC6644, 0.4),
            new MeridianBody.StatusEffect("contamination_slow", "外源侵蚀",
                "右臂残余外源真元未清除", 0xFF9944CC, 0.25)
        ));

        return b.build();
    }
}
