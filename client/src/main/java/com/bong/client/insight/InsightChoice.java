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
    String title,              // 短标题，<= 12 字 (e.g. "下次冲关稳")
    String effectSummary,      // 数值描述，便于决策 (e.g. "next_breakthrough_success_rate +5%")
    String flavor,             // agent 写的 1-3 句贴脸描述
    String styleHint           // 一行风格提示 (e.g. "保下一关")
) {
    public InsightChoice {
        Objects.requireNonNull(choiceId, "choiceId");
        Objects.requireNonNull(category, "category");
        Objects.requireNonNull(title, "title");
        Objects.requireNonNull(effectSummary, "effectSummary");
        Objects.requireNonNull(flavor, "flavor");
        styleHint = styleHint == null ? "" : styleHint;
    }
}
