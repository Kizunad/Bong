package com.bong.client.skill;

import org.junit.jupiter.api.Test;

import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SkillSetSnapshotTest {

    @Test
    void consumedScrollsArePreservedEvenWhenSkillsMapIsEmpty() {
        SkillSetSnapshot snapshot = SkillSetSnapshot.of(
            java.util.Map.of(),
            java.util.Set.of("skill_scroll_herbalism_baicao_can")
        );

        assertTrue(snapshot.hasConsumedScroll("skill_scroll_herbalism_baicao_can"));
    }

    @Test
    void maxEffectiveLevelReturnsZeroForEmptySnapshot() {
        assertEquals(0, SkillSetSnapshot.empty().maxEffectiveLv(),
            "空技能快照必须返回 0，作为技能门禁的 fail-closed 基线");
    }

    @Test
    void maxEffectiveLevelUsesCappedLevelsAndKeepsHighestSkill() {
        SkillSetSnapshot snapshot = SkillSetSnapshot.of(Map.of(
            SkillId.HERBALISM, new SkillSetSnapshot.Entry(8, 0, 100, 0, 3, 0, 0),
            SkillId.FORGING, new SkillSetSnapshot.Entry(7, 0, 100, 0, 7, 0, 0),
            SkillId.ALCHEMY, new SkillSetSnapshot.Entry(2, 0, 100, 0, 2, 0, 0)
        ));

        assertEquals(7, snapshot.maxEffectiveLv(),
            "技能门应比较 effectiveLv=min(lv,cap)，不能把采药真实 Lv.8 越过 cap 当作 8");
    }
}
