package com.bong.client.inventory;

import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillMilestoneSnapshot;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class InspectScreenSkillMilestoneTest {

    @Test
    void milestoneLinePrefersNarrationWhenPresent() {
        SkillMilestoneSnapshot milestone = new SkillMilestoneSnapshot(
            SkillId.HERBALISM,
            4,
            82000,
            "你摘得百草渐熟，今已识八分。",
            1400
        );

        assertEquals(
            "Lv.4 · 你摘得百草渐熟，今已识八分。",
            InspectScreen.formatSkillMilestoneLine(milestone)
        );
    }

    @Test
    void milestoneLineFallsBackToTickAndXpWithoutNarration() {
        SkillMilestoneSnapshot milestone = new SkillMilestoneSnapshot(
            SkillId.ALCHEMY,
            2,
            700,
            "",
            500
        );

        assertEquals(
            "Lv.2 · t700 · 累计 500 XP",
            InspectScreen.formatSkillMilestoneLine(milestone)
        );
    }
}
