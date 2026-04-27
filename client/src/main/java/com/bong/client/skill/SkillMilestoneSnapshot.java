package com.bong.client.skill;

/**
 * 活人端 `cultivation_detail` 里透传的 skill milestone 快照。
 */
public record SkillMilestoneSnapshot(
    SkillId skill,
    int newLv,
    long achievedAt,
    String narration,
    long totalXpAt
) {
    public SkillMilestoneSnapshot {
        newLv = Math.max(0, Math.min(10, newLv));
        achievedAt = Math.max(0L, achievedAt);
        narration = narration == null ? "" : narration;
        totalXpAt = Math.max(0L, totalXpAt);
    }
}
