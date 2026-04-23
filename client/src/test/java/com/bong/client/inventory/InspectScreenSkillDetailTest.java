package com.bong.client.inventory;

import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillSetSnapshot;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class InspectScreenSkillDetailTest {

    @Test
    void levelLineShowsCapSuppressionWhenRealLevelExceedsCap() {
        SkillSetSnapshot.Entry entry = new SkillSetSnapshot.Entry(7, 30, 1600, 3000, 5, 0, 0);

        assertEquals(
            "Lv.7 / effective 5 / cap 5 · 境界压制",
            InspectScreen.formatSkillLevelLine(SkillId.HERBALISM, entry)
        );
    }

    @Test
    void herbalismCurrentEffectReflectsEffectiveLevelNotRealLevel() {
        SkillSetSnapshot.Entry entry = new SkillSetSnapshot.Entry(7, 30, 1600, 3000, 5, 0, 0);

        String line = InspectScreen.formatSkillCurrentEffect(SkillId.HERBALISM, entry);

        assertTrue(line.contains("当前效果：手动采集 -1.0s"));
        assertTrue(line.contains("种子掉率 +10.0%"));
        assertTrue(line.contains("品质偏移 +15%"));
        assertTrue(line.contains("自动采集已开，时长 6.0s"));
    }

    @Test
    void nextEffectExplainsSuppressedLevelsWillUnlockAfterBreakthrough() {
        SkillSetSnapshot.Entry entry = new SkillSetSnapshot.Entry(7, 30, 1600, 3000, 5, 0, 0);

        assertEquals(
            "下一阶：真实等级已高于境界上限；待突破后，压住的效果会直接放开。",
            InspectScreen.formatSkillNextEffect(SkillId.ALCHEMY, entry)
        );
    }

    @Test
    void forgingCurrentEffectUsesPlanEndpoints() {
        SkillSetSnapshot.Entry entry = new SkillSetSnapshot.Entry(3, 0, 900, 1400, 10, 0, 0);

        assertEquals(
            "当前效果：淬火命中窗 +3 tick，允许失误 +1，铭文失败率 -10.0%。",
            InspectScreen.formatSkillCurrentEffect(SkillId.FORGING, entry)
        );
    }

    @Test
    void maxLevelProgressLineStopsShowingNextBucketXp() {
        SkillSetSnapshot.Entry entry = new SkillSetSnapshot.Entry(10, 0, 10000, 38500, 10, 0, 0);

        assertEquals("Lv.10 已满 · 累计 38500 XP", InspectScreen.formatSkillProgressLine(entry));
    }
}
