package com.bong.client.alchemy;

import com.bong.client.skill.SkillSetSnapshot;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class AlchemyScreenSkillHeaderTest {

    @Test
    void headerShowsEffectiveLevelAndToleranceBonus() {
        SkillSetSnapshot.Entry entry = new SkillSetSnapshot.Entry(4, 0, 1600, 3000, 10, 0, 0);

        assertEquals(
            "炼丹 Lv.4 · 本次火候容差 +20%",
            AlchemyScreen.formatAlchemySkillHeader(entry)
        );
    }

    @Test
    void headerShowsSuppressedEffectiveLevelWhenOverCap() {
        SkillSetSnapshot.Entry entry = new SkillSetSnapshot.Entry(7, 0, 1600, 3000, 5, 0, 0);

        assertEquals(
            "炼丹 Lv.7 (压制→5) · 本次火候容差 +25%",
            AlchemyScreen.formatAlchemySkillHeader(entry)
        );
    }
}
