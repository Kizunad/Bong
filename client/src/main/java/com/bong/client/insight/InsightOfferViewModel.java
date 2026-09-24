package com.bong.client.insight;

import java.util.List;
import java.util.Objects;

/**
 * 一次顿悟邀约的快照——server 通过 InsightOfferStore 推到客户端。
 *
 * <p>包含 trigger 上下文（让玩家明白为何此刻顿悟）+ 2-3 个候选 + 截止时间。
 *
 * <p>{@code offerId} 是本次 offer 实例的唯一 settlement identity（wire {@code offer_id}，
 * server 形如 {@code insight:{entity}:{trigger}} / {@code heart_demon:...}）；{@code triggerId}
 * 只作为可复用的触发上下文——同一 trigger 的先后两份 offer 是不同实例（r7-insight-settlement.tsv
 * identity_rule）。旧客户端 payload 缺 {@code offer_id} 时由 handler 用 triggerId 派生，保证
 * 实例身份恒非空。
 */
public record InsightOfferViewModel(
    String offerId,              // 实例唯一 identity（settlement claim 与 compare-and-clear 的键）
    String triggerId,            // 可复用触发上下文（wire trigger_id；C2S 决定仍只带它）
    String triggerLabel,         // 已本地化的触发描述 (e.g. "首次突破到引气境")
    String realmLabel,           // 当前境界中文名 (e.g. "引气境 (3 正经)")
    double composure,            // 0-1
    int quotaRemaining,          // 当前境界剩余顿悟额度 (含本次)
    int quotaTotal,              // 当前境界总额度
    long expiresAtMillis,        // 客户端 wall-clock 截止时刻
    List<InsightChoice> choices  // 2-3 项
) {
    public InsightOfferViewModel {
        Objects.requireNonNull(offerId, "offerId");
        if (offerId.isBlank()) {
            throw new IllegalArgumentException("offerId 不可为空——offer 实例身份缺失");
        }
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
