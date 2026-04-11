package com.bong.client.inventory.model;

/**
 * 单个身体部位的状态。
 *
 * @param part         部位
 * @param wound        伤势等级
 * @param bleedRate    出血速率 0~1（0=不出血，1=大出血）
 * @param healProgress 恢复进度 0~1
 * @param splinted     是否已上夹板（骨折时有效）
 */
public record BodyPartState(
    BodyPart part,
    WoundLevel wound,
    double bleedRate,
    double healProgress,
    boolean splinted
) {
    public BodyPartState {
        bleedRate = Math.max(0, Math.min(1, bleedRate));
        healProgress = Math.max(0, Math.min(1, healProgress));
    }

    public static BodyPartState intact(BodyPart part) {
        return new BodyPartState(part, WoundLevel.INTACT, 0, 0, false);
    }
}
