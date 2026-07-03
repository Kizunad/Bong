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
    MERIDIAN_GATED,

    // ─── 通用技能警示（plan-skill-warn-hud）：施放前 resolver 拒绝原因 ───────────
    // 与服务端 CastOutcomeV1::Reject* / proto CAST_OUTCOME_REJECT_* 1:1。
    // 纯反馈：cast 已被服务端 resolver 拒绝，这些 outcome 只驱动通用警示 HUD 文案。
    /** 真元不足。 */
    REJECT_QI_INSUFFICIENT,
    /** 招式冷却中。 */
    REJECT_ON_COOLDOWN,
    /** 目标无效（无目标 / 超距 / 功法未激活等）。 */
    REJECT_INVALID_TARGET,
    /** 处于硬直 / 恢复期。 */
    REJECT_IN_RECOVERY,
    /** 境界不足。 */
    REJECT_REALM_TOO_LOW,
    /** 缺少所需武器。 */
    REJECT_NO_WEAPON,
    /** 招式未习得或未激活（此前冒用 REJECT_INVALID_TARGET）。 */
    REJECT_TECHNIQUE_INACTIVE;

    public boolean consumesItem() {
        return this == COMPLETED;
    }

    public boolean isInterrupt() {
        return switch (this) {
            case INTERRUPT_MOVEMENT, INTERRUPT_CONTAM, INTERRUPT_CONTROL, USER_CANCEL, DEATH -> true;
            // MERIDIAN_GATED 与所有 REJECT_* 都是施放前拒绝（非进行中 cast 被打断），
            // 归入 interrupt 分类让 HUD 显示拒绝反馈而非静默忽略。
            case MERIDIAN_GATED, REJECT_QI_INSUFFICIENT, REJECT_ON_COOLDOWN,
                 REJECT_INVALID_TARGET, REJECT_IN_RECOVERY, REJECT_REALM_TOO_LOW,
                 REJECT_NO_WEAPON, REJECT_TECHNIQUE_INACTIVE -> true;
            default -> false;
        };
    }

    /**
     * 施放前拒绝（含经脉门控与所有 resolver Reject*）——通用技能警示 HUD 据此弹瞬态提示。
     * 与 {@link #isInterrupt()} 区别：interrupt 还含进行中 cast 被运动/控场打断；
     * 本谓词只圈"施放前因前置不满足被拒"，是警示文案映射的入口判定。
     */
    public boolean isCastRejection() {
        return switch (this) {
            case MERIDIAN_GATED, REJECT_QI_INSUFFICIENT, REJECT_ON_COOLDOWN,
                 REJECT_INVALID_TARGET, REJECT_IN_RECOVERY, REJECT_REALM_TOO_LOW,
                 REJECT_NO_WEAPON, REJECT_TECHNIQUE_INACTIVE -> true;
            default -> false;
        };
    }

    /**
     * 通用警示 HUD 的中文友好文案。覆盖全部拒绝原因；非拒绝 outcome 返回 null
     * （调用方据 null 跳过弹提示，保证 HUD 瞬态不常驻）。
     */
    public String warningText() {
        return switch (this) {
            case MERIDIAN_GATED -> "经脉受损";
            case REJECT_QI_INSUFFICIENT -> "真元不足";
            case REJECT_ON_COOLDOWN -> "冷却中";
            case REJECT_INVALID_TARGET -> "目标无效";
            case REJECT_IN_RECOVERY -> "尚未恢复";
            case REJECT_REALM_TOO_LOW -> "境界不足";
            case REJECT_NO_WEAPON -> "缺少武器";
            case REJECT_TECHNIQUE_INACTIVE -> "招式未激活";
            default -> null;
        };
    }
}
