package com.bong.client.insight;

import java.util.List;

/**
 * 调试/无服务端数据时的样例邀约。对应 docs/plan-cultivation-v1.md §5.2 中
 * "首次突破到引气境 (first_breakthrough_to_Induce)" trigger 的 3 个候选 (E1 / C3 / G1)。
 */
public final class MockInsightOfferData {
    private MockInsightOfferData() {
    }

    public static InsightOfferViewModel firstInduceBreakthrough() {
        return firstInduceBreakthrough(System.currentTimeMillis() + 60_000L);
    }

    public static InsightOfferViewModel firstInduceBreakthrough(long expiresAtMillis) {
        return new InsightOfferViewModel(
            "first_breakthrough_to_Induce",
            "首次突破到引气境",
            "引气境 (3 正经)",
            0.78,
            2,
            2,
            expiresAtMillis,
            List.of(
                new InsightChoice(
                    "mock_choice_E1",
                    InsightCategory.BREAKTHROUGH,
                    InsightAlignment.CONVERGE,
                    "下次冲关稳",
                    "next_breakthrough_success_rate +5% (一次性)",
                    "沉重色效率 -15%",
                    "你已知冲关时神识凝聚的诀窍，下次心会更稳。",
                    "越专精越偏科，沉重一路会变得生涩。",
                    "保下一关"
                ),
                new InsightChoice(
                    "mock_choice_C3",
                    InsightCategory.COMPOSURE,
                    InsightAlignment.NEUTRAL,
                    "闭关心如止",
                    "composure_immune_during BreakthroughState",
                    "心境冲击敏感 +3%",
                    "闭关时外界纷扰再不能扰你——下次突破基线提升。",
                    "心湖恢复更快，也更容易被冲击搅动。",
                    "稀有强力，提升突破基线"
                ),
                new InsightChoice(
                    "mock_choice_G1",
                    InsightCategory.PERCEPTION,
                    InsightAlignment.DIVERGE,
                    "灵气浓淡可见",
                    "unlock_perception zone_qi_density (方圆 100m)",
                    "锋锐色效率 -10%",
                    "你能感知方圆百米灵气浓淡，再不会盲目静坐于枯地。",
                    "转向感知会让旧有锋锐肌肉记忆褪去。",
                    "战略侦察"
                )
            )
        );
    }

    public static InsightOfferViewModel heartDemonOffer() {
        return new InsightOfferViewModel(
            "heart_demon:1:1000",
            "心魔劫临身",
            "渡虚劫 · 心魔",
            0.5,
            1,
            1,
            System.currentTimeMillis() + 30_000L,
            List.of(
                new InsightChoice(
                    "heart_demon_choice_0",
                    InsightCategory.COMPOSURE,
                    "守本心",
                    "回复少量当前真元",
                    "你把呼吸压回丹田。",
                    "稳妥"
                ),
                new InsightChoice(
                    "heart_demon_choice_1",
                    InsightCategory.BREAKTHROUGH,
                    "斩执念",
                    "失败则强化下一雷",
                    "刀锋照见自己的影。",
                    "冒险"
                ),
                new InsightChoice(
                    "heart_demon_choice_2",
                    InsightCategory.PERCEPTION,
                    "无解",
                    "不增益也不受真元惩罚",
                    "你不再替天道补题。",
                    "止损"
                )
            )
        );
    }
}
