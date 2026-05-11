package com.bong.client.insight;

import java.util.Objects;

/**
 * 一个顿悟选项——agent 输出经 Arbiter 校验后下发给 client。
 *
 * <p>客户端不解释 effectId / magnitude 的语义，只忠实展示给玩家。校验 / 应用都在 server。
 */
public record InsightChoice(
    String choiceId,           // server 生成的稳定 id，回传时使用
    InsightCategory category,
    InsightAlignment alignment,
    String title,              // 短标题，<= 12 字 (e.g. "下次冲关稳")
    String effectSummary,      // 数值描述，便于决策 (e.g. "next_breakthrough_success_rate +5%")
    String costSummary,        // 代价短描述，必须常显
    String flavor,             // agent 写的 1-3 句贴脸描述
    String costFlavor,         // 代价叙事描述，红字展示
    String styleHint           // 一行风格提示 (e.g. "保下一关")
) {
    public InsightChoice(String choiceId,
                         InsightCategory category,
                         String title,
                         String effectSummary,
                         String flavor,
                         String styleHint) {
        this(
            choiceId,
            category,
            InsightAlignment.NEUTRAL,
            title,
            effectSummary,
            "真元挥发 +1%",
            flavor,
            "气机更活，战斗中真元更易挥发。",
            styleHint
        );
    }

    public InsightChoice {
        Objects.requireNonNull(choiceId, "choiceId");
        Objects.requireNonNull(category, "category");
        alignment = alignment == null ? InsightAlignment.NEUTRAL : alignment;
        Objects.requireNonNull(title, "title");
        Objects.requireNonNull(effectSummary, "effectSummary");
        costSummary = costSummary == null || costSummary.isBlank() ? "代价待结算" : costSummary;
        Objects.requireNonNull(flavor, "flavor");
        costFlavor = costFlavor == null || costFlavor.isBlank() ? costSummary : costFlavor;
        styleHint = styleHint == null ? "" : styleHint;
    }
}
