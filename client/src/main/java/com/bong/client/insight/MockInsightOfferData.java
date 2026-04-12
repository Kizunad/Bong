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
                    "下次冲关稳",
                    "next_breakthrough_success_rate +5% (一次性)",
                    "你已知冲关时神识凝聚的诀窍，下次心会更稳。",
                    "保下一关"
                ),
                new InsightChoice(
                    "mock_choice_C3",
                    InsightCategory.COMPOSURE,
                    "闭关心如止",
                    "composure_immune_during BreakthroughState",
                    "闭关时外界纷扰再不能扰你——下次突破基线提升。",
                    "稀有强力，提升突破基线"
                ),
                new InsightChoice(
                    "mock_choice_G1",
                    InsightCategory.PERCEPTION,
                    "灵气浓淡可见",
                    "unlock_perception zone_qi_density (方圆 100m)",
                    "你能感知方圆百米灵气浓淡，再不会盲目静坐于枯地。",
                    "战略侦察"
                )
            )
        );
    }
}
