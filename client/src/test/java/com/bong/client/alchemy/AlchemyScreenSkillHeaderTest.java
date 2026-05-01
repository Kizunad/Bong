package com.bong.client.alchemy;

import com.bong.client.skill.SkillSetSnapshot;
import com.bong.client.inventory.model.InventoryItem;
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

    @Test
    void feedCountUsesRecipeRequirementInsteadOfSingleItem() {
        InventoryItem ciSheHaoStack = InventoryItem.createFull(
            1L, "ci_she_hao", "刺蛇蒿", 1, 1, 0.1, "common", "", 8, 1.0, 1.0
        );
        InventoryItem huiYuanZhiStack = InventoryItem.createFull(
            2L, "hui_yuan_zhi", "回元芝", 1, 1, 0.1, "common", "", 8, 1.0, 1.0
        );
        InventoryItem chiSuiCaoStack = InventoryItem.createFull(
            3L, "chi_sui_cao", "赤髓草", 1, 1, 0.1, "common", "", 8, 1.0, 1.0
        );

        assertEquals(3, AlchemyScreen.feedCountForSlot("kaimai_pill", 0, ciSheHaoStack));
        assertEquals(2, AlchemyScreen.feedCountForSlot("hui_yuan_pill_v0", 0, huiYuanZhiStack));
        assertEquals(4, AlchemyScreen.feedCountForSlot("du_ming_san_v0", 0, chiSuiCaoStack));
    }

    @Test
    void feedCountFallsBackToAvailableStackWhenRequirementIsLarger() {
        InventoryItem shortStack = InventoryItem.createFull(
            4L, "ci_she_hao", "刺蛇蒿", 1, 1, 0.1, "common", "", 2, 1.0, 1.0
        );

        assertEquals(2, AlchemyScreen.feedCountForSlot("kai_mai_pill_v0", 0, shortStack));
    }
}
