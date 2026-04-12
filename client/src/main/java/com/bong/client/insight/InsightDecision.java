package com.bong.client.insight;

import java.util.Objects;

/**
 * 玩家对一次顿悟邀约做出的决定 (选 / 拒 / 超时)。
 *
 * <p>统一封装供 {@link InsightChoiceDispatcher} 序列化并回传服务端。
 *
 * <p>字段 {@code triggerId} 对应 server 的 {@code trigger_id}，也是一次邀约的
 * 唯一标识（一个 trigger → 一份 PendingInsightOffer → 一次决定）。
 */
public record InsightDecision(
    String triggerId,
    Kind kind,
    String chosenChoiceId    // 仅 CHOSEN 时非 null
) {
    public InsightDecision {
        Objects.requireNonNull(triggerId, "triggerId");
        Objects.requireNonNull(kind, "kind");
        if (kind == Kind.CHOSEN) {
            Objects.requireNonNull(chosenChoiceId, "chosenChoiceId required when kind=CHOSEN");
        } else if (chosenChoiceId != null) {
            throw new IllegalArgumentException("chosenChoiceId must be null when kind=" + kind);
        }
    }

    public static InsightDecision chosen(String triggerId, String choiceId) {
        return new InsightDecision(triggerId, Kind.CHOSEN, choiceId);
    }

    public static InsightDecision declined(String triggerId) {
        return new InsightDecision(triggerId, Kind.DECLINED, null);
    }

    public static InsightDecision timedOut(String triggerId) {
        return new InsightDecision(triggerId, Kind.TIMED_OUT, null);
    }

    public String summary() {
        return switch (kind) {
            case CHOSEN -> "CHOSEN " + chosenChoiceId;
            case DECLINED -> "DECLINED";
            case TIMED_OUT -> "TIMED_OUT";
        };
    }

    public enum Kind {
        CHOSEN,
        DECLINED,
        TIMED_OUT
    }
}
