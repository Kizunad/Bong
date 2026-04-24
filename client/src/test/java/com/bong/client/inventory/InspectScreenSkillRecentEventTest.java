package com.bong.client.inventory;

import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillRecentEventStore;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class InspectScreenSkillRecentEventTest {

    @Test
    void recentEventLineUsesStoredText() {
        SkillRecentEventStore.Entry entry = new SkillRecentEventStore.Entry(
            SkillId.ALCHEMY,
            "xp_gain",
            "+6 XP",
            1234L
        );

        assertEquals("+6 XP", InspectScreen.formatSkillRecentEventLine(entry));
    }

    @Test
    void recentEventLineSupportsLevelUpSummary() {
        SkillRecentEventStore.Entry entry = new SkillRecentEventStore.Entry(
            SkillId.FORGING,
            "lv_up",
            "升至 Lv.4",
            1234L
        );

        assertEquals("升至 Lv.4", InspectScreen.formatSkillRecentEventLine(entry));
    }
}
