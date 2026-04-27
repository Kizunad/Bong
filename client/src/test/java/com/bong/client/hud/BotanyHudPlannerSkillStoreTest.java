package com.bong.client.hud;

import com.bong.client.skill.SkillId;
import com.bong.client.skill.SkillSetSnapshot;
import com.bong.client.skill.SkillSetStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;

class BotanyHudPlannerSkillStoreTest {

    @AfterEach
    void tearDown() {
        SkillSetStore.resetForTests();
    }

    @Test
    void herbalismHudUsesEffectiveLevelInsteadOfSuppressedRealLevel() {
        SkillSetStore.replace(SkillSetSnapshot.empty().withSkill(
            SkillId.HERBALISM,
            new SkillSetSnapshot.Entry(4, 40, 100, 3000, 2, 0, 0)
        ));

        var view = BotanyHudPlanner.herbalismView();

        assertEquals(2, view.level());
        assertFalse(view.autoUnlocked(), "effective lv 2 should still keep auto harvest locked");
    }
}
