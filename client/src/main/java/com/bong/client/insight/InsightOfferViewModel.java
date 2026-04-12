package com.bong.client.insight;

import java.util.List;
import java.util.Objects;

/**
 * 一次顿悟邀约的快照——server 通过 InsightOfferStore 推到客户端。
 *
 * <p>包含 trigger 上下文（让玩家明白为何此刻顿悟）+ 2-3 个候选 + 截止时间。
 */
public record InsightOfferViewModel(
    String triggerId,            // 同时作为 offer 唯一标识（一个 trigger → 一份 offer）
    String triggerLabel,         // 已本地化的触发描述 (e.g. "首次突破到引气境")
    String realmLabel,           // 当前境界中文名 (e.g. "引气境 (3 正经)")
    double composure,            // 0-1
    int quotaRemaining,          // 当前境界剩余顿悟额度 (含本次)
    int quotaTotal,              // 当前境界总额度
    long expiresAtMillis,        // 客户端 wall-clock 截止时刻
    List<InsightChoice> choices  // 2-3 项
) {
    public InsightOfferViewModel {
        Objects.requireNonNull(triggerId, "triggerId");
        Objects.requireNonNull(triggerLabel, "triggerLabel");
        Objects.requireNonNull(realmLabel, "realmLabel");
        Objects.requireNonNull(choices, "choices");
        if (choices.isEmpty() || choices.size() > 4) {
            throw new IllegalArgumentException("顿悟选项数量必须为 1-4，实际: " + choices.size());
        }
        choices = List.copyOf(choices);
    }

    /** 距过期还有多少毫秒 (永不为负，过期时返回 0)。 */
    public long remainingMillis(long nowMillis) {
        return Math.max(0L, expiresAtMillis - nowMillis);
    }

    /** 是否已过期。 */
    public boolean isExpired(long nowMillis) {
        return nowMillis >= expiresAtMillis;
    }
}
