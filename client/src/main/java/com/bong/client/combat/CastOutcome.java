package com.bong.client.combat;

/**
 * Reason a cast terminated (§4.4 — drives item refund policy).
 */
public enum CastOutcome {
    NONE,
    COMPLETED,
    INTERRUPT_MOVEMENT,
    INTERRUPT_CONTAM,
    INTERRUPT_CONTROL,
    USER_CANCEL,
    DEATH,
    /**
     * 施放前被经脉门控拒绝：经脉未打通 / 已断 / integrity 不足。
     * 对应服务端 CastOutcomeV1::MeridianGated（serde snake_case = "meridian_gated"）。
     * 与 USER_CANCEL 区别：玩家并未主动取消，而是经脉状态不满足。
     */
    MERIDIAN_GATED;

    public boolean consumesItem() {
        return this == COMPLETED;
    }

    public boolean isInterrupt() {
        return switch (this) {
            case INTERRUPT_MOVEMENT, INTERRUPT_CONTAM, INTERRUPT_CONTROL, USER_CANCEL, DEATH -> true;
            // MERIDIAN_GATED 是施放前拒绝（非进行中 cast 被打断），归入 interrupt 分类
            // 让 HUD 显示拒绝反馈而非静默忽略
            case MERIDIAN_GATED -> true;
            default -> false;
        };
    }
}
